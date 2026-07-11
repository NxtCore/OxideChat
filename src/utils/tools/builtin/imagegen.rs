use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use omniference::types::{ImageInput, ImageOperation, ImageOptions, ImageRequestIR, ModelRef};
use reqwest::{Client, header::CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use super::super::executor::{ToolContext, ToolError, ToolExecutor};
use crate::ai::provider_to_config;
use crate::types::models::Model;
use crate::types::providers::{Provider, ProviderKind};
use crate::utils::images::{image_url, safe_image_mime, store_image};

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
	image_url: String,
	prompt: String,
}

pub struct ImageGenExecutor {
	client: Client,
}

impl ImageGenExecutor {
	pub fn new() -> Result<Self, ToolError> {
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|error| ToolError::Internal(format!("Failed to create HTTP client: {error}")))?;
		Ok(Self { client })
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
		if !model.capabilities.0.iter().any(|capability| capability == "IMAGE_GENERATION") || !model.output_modalities.0.iter().any(|modality| modality.eq_ignore_ascii_case("IMAGE")) {
			return Err(ToolError::ExecutionFailed("Configured model does not support image generation".to_string()));
		}
		if let Some(user_id) = ctx.user_id {
			let allowed = Model::can_user_use_model(db, &user_id, &model.id)
				.await
				.map_err(|error| ToolError::Internal(format!("Failed to check image-model access: {error}")))?;
			if !allowed {
				return Err(ToolError::ExecutionFailed("You do not have access to the configured image model".to_string()));
			}
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

		let response = self.client.get(value).send().await.map_err(|error| ToolError::HttpError(format!("Failed to download image: {error}")))?;
		if !response.status().is_success() { return Err(ToolError::HttpError(format!("Failed to download image: HTTP {}", response.status()))); }
		let mime = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).and_then(|value| value.split(';').next()).unwrap_or_default().to_string();
		let bytes = response.bytes().await.map_err(|error| ToolError::HttpError(format!("Failed to read image: {error}")))?.to_vec();
		let media_type = safe_image_mime(&bytes, &mime).ok_or_else(|| ToolError::InvalidInput("Unsupported image content".to_string()))?;
		Ok(ImageInput { bytes, media_type: media_type.to_string() })
	}

	async fn store_response(&self, response: omniference::types::ImageResponse, ctx: &ToolContext) -> Result<Value, ToolError> {
		let db = ctx.db.as_ref().ok_or_else(|| ToolError::Internal("Database not available".to_string()))?;
		let (bytes, declared_mime) = response.images.into_iter().next().ok_or_else(|| ToolError::ExecutionFailed("Image provider returned no images".to_string()))?;
		let mime = safe_image_mime(&bytes, &declared_mime).ok_or_else(|| ToolError::ExecutionFailed("Image provider returned unsupported image content".to_string()))?;
		let stored = store_image(db, &bytes, mime, ctx.user_id, Some("imagegen")).await.map_err(ToolError::Internal)?;
		Ok(json!({"success": true, "image_url": image_url(stored.id)}))
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
				(input.prompt, vec![self.input_image(&input.image_url, ctx).await?], ImageOptions::default())
			}
		};
		let request = ImageRequestIR {
			model: ModelRef { alias: model.display_name.clone(), provider: provider_to_config(&provider), model_id: model.model_id, input_modalities: Vec::new(), output_modalities: Vec::new() },
			operation,
			prompt,
			input_images,
			options,
		};
		let engine = crate::ai::get();
		let response = engine.read().await.image(request).await.map_err(ToolError::ExecutionFailed)?;
		self.store_response(response, ctx).await
	}

	fn name(&self) -> &str { "imagegen" }
}
