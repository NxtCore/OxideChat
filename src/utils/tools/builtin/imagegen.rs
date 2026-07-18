use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::StreamExt;
use omniference::types::{ImageInput, ImageOperation, ImageOptions, ImageRequestIR, ImageResponse, ModelRef};
use reqwest::{Client, Url, header::CONTENT_TYPE, redirect::Policy};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::IpAddr, time::Duration};
use uuid::Uuid;

use super::super::executor::{ToolContext, ToolError, ToolExecutor};
use crate::ai::provider_to_config;
use crate::types::budgets::Budget;
use crate::types::models::{Model, ModelPricing};
use crate::types::providers::{Provider, ProviderKind};
use crate::types::usage::{UsageEvent, UsageEventRecord};
use crate::utils::images::{safe_image_mime, store_image};

#[derive(Debug, Deserialize)]
struct GenerateImageInput {
	prompt: String,
	#[serde(default)]
	size: Option<String>,
	#[serde(default)]
	quality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EditImageInput {
	#[serde(default)]
	image_id: Option<String>,
	#[serde(default)]
	image_url: Option<String>,
	prompt: String,
}

const MAX_REMOTE_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub struct ImageGenExecutor;

fn is_blocked_address(address: IpAddr) -> bool {
	match address {
		IpAddr::V4(address) => address.is_private() || address.is_loopback() || address.is_link_local() || address.is_multicast() || address.is_unspecified() || address.is_broadcast(),
		IpAddr::V6(address) => {
			address.to_ipv4_mapped().is_some_and(|address| is_blocked_address(IpAddr::V4(address)))
				|| address.is_unique_local()
				|| address.is_loopback()
				|| address.is_unicast_link_local()
				|| address.is_multicast()
				|| address.is_unspecified()
		}
	}
}

impl ImageGenExecutor {
	pub fn new() -> Result<Self, ToolError> {
		Ok(Self)
	}

	async fn load_model(&self, model_id: &str, ctx: &ToolContext) -> Result<(Model, Provider), ToolError> {
		let db = ctx.db.as_ref().ok_or_else(|| ToolError::Internal("Database not available".to_string()))?;
		let id = Uuid::parse_str(model_id).map_err(|_| ToolError::InvalidInput("image_model_id must be a UUID".to_string()))?;
		let model = Model::find_by_id(db, &id)
			.await
			.map_err(|error| ToolError::Internal(format!("Failed to load image model: {error}")))?
			.ok_or_else(|| ToolError::InvalidInput("Configured image model no longer exists".to_string()))?;
		let provider = Provider::find_for_admin(db, &model.provider_id)
			.await
			.map_err(|error| ToolError::Internal(format!("Failed to load image provider: {error}")))?
			.ok_or_else(|| ToolError::InvalidInput("Configured image provider no longer exists".to_string()))?;
		if !model.is_enabled || !provider.is_enabled {
			return Err(ToolError::ExecutionFailed("Configured image model is disabled".to_string()));
		}
		if !matches!(provider.kind, ProviderKind::Openai | ProviderKind::Openrouter | ProviderKind::Google) {
			return Err(ToolError::ExecutionFailed("Configured provider does not support image generation".to_string()));
		}
		if !model.output_modalities.0.iter().any(|modality| modality.eq_ignore_ascii_case("IMAGE")) {
			return Err(ToolError::ExecutionFailed("Configured model does not support image generation".to_string()));
		}
		Ok((model, provider))
	}

	async fn input_image(&self, value: &str, ctx: &ToolContext) -> Result<ImageInput, ToolError> {
		if let Some(data) = value.strip_prefix("data:") {
			let (metadata, encoded) = data.split_once(',').ok_or_else(|| ToolError::InvalidInput("Invalid image data URL".to_string()))?;
			if !metadata.split(';').any(|part| part.eq_ignore_ascii_case("base64")) {
				return Err(ToolError::InvalidInput("Image data URL must use base64".to_string()));
			}
			let mime = metadata.split(';').next().unwrap_or_default().to_string();
			let bytes = BASE64.decode(encoded).map_err(|error| ToolError::InvalidInput(format!("Invalid image data: {error}")))?;
			let media_type = safe_image_mime(&bytes, &mime).ok_or_else(|| ToolError::InvalidInput("Unsupported image content".to_string()))?;
			return Ok(ImageInput { bytes, media_type: media_type.to_string() });
		}

		if let Some(id) = value.strip_prefix("/api/v1/images/") {
			let db = ctx.db.as_ref().ok_or_else(|| ToolError::Internal("Database not available".to_string()))?;
			let id = Uuid::parse_str(id).map_err(|_| ToolError::InvalidInput("Invalid image ID".to_string()))?;
			let (bytes, mime) = crate::utils::images::get_image(db, id).await.map_err(ToolError::Internal)?.ok_or_else(|| ToolError::InvalidInput("Image not found".to_string()))?;
			let media_type = safe_image_mime(&bytes, &mime).ok_or_else(|| ToolError::InvalidInput("Unsupported image content".to_string()))?;
			return Ok(ImageInput { bytes, media_type: media_type.to_string() });
		}

		let fetch_error = || ToolError::HttpError("Failed to fetch image".to_string());
		let url = Url::parse(value).map_err(|_| fetch_error())?;
		if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() {
			return Err(fetch_error());
		}
		let host = url.host_str().ok_or_else(fetch_error)?.to_string();
		let port = url.port_or_known_default().ok_or_else(fetch_error)?;
		let addresses: Vec<_> = tokio::net::lookup_host((host.as_str(), port)).await.map_err(|_| fetch_error())?.collect();
		if addresses.is_empty() || addresses.iter().any(|address| is_blocked_address(address.ip())) {
			return Err(fetch_error());
		}
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.redirect(Policy::none())
			.resolve(&host, addresses[0])
			.build()
			.map_err(|_| fetch_error())?;
		let response = client.get(url).send().await.map_err(|_| fetch_error())?;
		if !response.status().is_success() || response.remote_addr().is_some_and(|address| is_blocked_address(address.ip())) {
			return Err(fetch_error());
		}
		if response.content_length().is_some_and(|length| length > MAX_REMOTE_IMAGE_BYTES as u64) {
			return Err(fetch_error());
		}
		let mime = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).and_then(|value| value.split(';').next()).unwrap_or_default().to_string();
		let mut bytes = Vec::with_capacity(response.content_length().unwrap_or(0).min(MAX_REMOTE_IMAGE_BYTES as u64) as usize);
		let mut stream = response.bytes_stream();
		while let Some(chunk) = stream.next().await {
			let chunk = chunk.map_err(|_| fetch_error())?;
			if bytes.len().saturating_add(chunk.len()) > MAX_REMOTE_IMAGE_BYTES {
				return Err(fetch_error());
			}
			bytes.extend_from_slice(&chunk);
		}
		let media_type = safe_image_mime(&bytes, &mime).ok_or_else(fetch_error)?;
		Ok(ImageInput { bytes, media_type: media_type.to_string() })
	}

	async fn record_usage(&self, ctx: &ToolContext, model: &Model, usage: &omniference::types::ImageUsage) {
		let Some(db) = ctx.db.as_ref() else { return };
		let Some(user_id) = ctx.user_id else { return };
		let input_tokens = usage.input_tokens.min(i32::MAX as u64) as i32;
		let output_tokens = usage.output_tokens.min(i32::MAX as u64) as i32;
		let cost_total = if let Some(cost) = usage.provider_cost {
			Decimal::from_f64(cost).unwrap_or(Decimal::ZERO)
		} else {
			match ModelPricing::usage_cost(db, &model.id, input_tokens, output_tokens, 0).await {
				Ok(Some(cost)) => cost,
				_ => Decimal::ZERO,
			}
		};
		let team_id = Budget::primary_team_id(db, &user_id).await.ok().flatten();
		if let Err(error) = UsageEvent::record(
			db,
			UsageEventRecord {
				user_id: &user_id,
				team_id,
				model_id: &model.id,
				provider_id: &model.provider_id,
				request_type: "image",
				input_tokens,
				output_tokens,
				reasoning_tokens: 0,
				cost_total,
			},
		)
		.await
		{
			tracing::error!(%error, "Failed to record image generation usage event");
		}
	}

	async fn store_response(&self, response: ImageResponse, ctx: &ToolContext, caption: &str) -> Result<Value, ToolError> {
		let db = ctx.db.as_ref().ok_or_else(|| ToolError::Internal("Database not available".to_string()))?;
		let image = response.images.into_iter().next().ok_or_else(|| ToolError::ExecutionFailed("Image provider returned no images".to_string()))?;
		let mime = safe_image_mime(&image.bytes, &image.media_type).ok_or_else(|| ToolError::ExecutionFailed("Image provider returned unsupported image content".to_string()))?;
		let stored = store_image(db, &image.bytes, mime, ctx.user_id, Some("imagegen")).await.map_err(ToolError::Internal)?;
		if !caption.is_empty() {
			if let Err(error) = crate::utils::images::set_image_caption(db, stored.id, caption).await {
				tracing::error!(image_id = %stored.id, %error, "Failed to store image caption");
			}
		}
		Ok(json!({"success": true, "image_id": stored.id, "image_is_shown_to_user": true, "show_image_with_markdown_in_chat": false}))
	}
}

#[async_trait]
impl ToolExecutor for ImageGenExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		let model_id = ctx.settings.get("image_model_id").and_then(Value::as_str).ok_or_else(|| ToolError::MissingSetting("image_model_id".to_string()))?;
		let operation = match ctx.function_name.as_deref() {
			None | Some("generate" | "imagegen") => ImageOperation::Generate,
			Some("edit") => ImageOperation::Edit,
			Some(other) => return Err(ToolError::InvalidInput(format!("Unknown image operation: {other}"))),
		};
		let (model, provider) = self.load_model(model_id, ctx).await?;
		if operation == ImageOperation::Edit && !model.capabilities.0.iter().any(|capability| capability == "IMAGE_EDITING") {
			return Err(ToolError::ExecutionFailed("Configured model does not support image editing".to_string()));
		}
		let (prompt, input_images, options) = match operation {
			ImageOperation::Generate => {
				let input: GenerateImageInput = serde_json::from_value(input).map_err(|error| ToolError::InvalidInput(error.to_string()))?;
				(input.prompt, Vec::new(), ImageOptions { size: input.size, quality: input.quality, ..ImageOptions::default() })
			}
			ImageOperation::Edit => {
				let input: EditImageInput = serde_json::from_value(input).map_err(|error| ToolError::InvalidInput(error.to_string()))?;
				let reference = if let Some(id) = input.image_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
					format!("/api/v1/images/{id}")
				} else if let Some(url) = input.image_url.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
					url.to_string()
				} else {
					return Err(ToolError::InvalidInput("Provide image_id (preferred) or image_url of the image to edit".to_string()));
				};
				(input.prompt, vec![self.input_image(&reference, ctx).await?], ImageOptions::default())
			}
		};
		let caption = prompt.clone();
		let provider_config = provider_to_config(&provider).map_err(|error| ToolError::Internal(error.to_string()))?;
		let request = ImageRequestIR {
			model: ModelRef { alias: model.display_name.clone(), provider: provider_config, model_id: model.model_id.clone(), input_modalities: Vec::new(), output_modalities: Vec::new() },
			operation,
			prompt,
			request_id: None,
			input_images,
			options,
		};
		let engine = crate::ai::get();
		let response = engine.read().await.image(request).await.map_err(ToolError::ExecutionFailed)?;
		self.record_usage(ctx, &model, &response.usage).await;
		self.store_response(response, ctx, &caption).await
	}

	fn name(&self) -> &str { "imagegen" }
}
