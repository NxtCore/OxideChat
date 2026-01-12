use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use super::super::executor::{ToolContext, ToolError, ToolExecutor};
use crate::utils::images::{image_url, store_from_data_uri};

const OPENAI_GENERATIONS_URL: &str = "https://api.openai.com/v1/images/generations";
const OPENAI_EDITS_URL: &str = "https://api.openai.com/v1/images/edits";
const REPLICATE_API_BASE: &str = "https://api.replicate.com/v1/models";
const GOOGLE_GEMINI_GENERATIONS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai/images/generations";
const GOOGLE_GEMINI_CONTENT_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateImageInput {
	pub prompt: String,
	#[serde(default = "default_size")]
	pub size: String,
	#[serde(default = "default_quality")]
	pub quality: String,
}

fn default_size() -> String {
	"1024x1024".to_string()
}

fn default_quality() -> String {
	"standard".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EditImageInput {
	pub image_url: String,
	pub prompt: String,
}

#[derive(Debug, Serialize)]
pub struct ImageResult {
	pub success: bool,
	pub image_url: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageResponse {
	data: Vec<OpenAIImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageData {
	#[serde(default)]
	url: Option<String>,
	#[serde(default)]
	b64_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReplicateResponse {
	output: Option<serde_json::Value>,
	error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContentResponse {
	candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
	content: Option<GeminiContent>,
	#[serde(rename = "finishReason")]
	finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
	parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
	#[serde(default)]
	text: Option<String>,
	#[serde(rename = "inlineData")]
	inline_data: Option<GeminiInlineData>,
}

#[derive(Debug, Deserialize)]
struct GeminiInlineData {
	#[serde(rename = "mimeType")]
	mime_type: String,
	data: String,
}

pub struct ImageGenExecutor {
	client: Client,
}

impl ImageGenExecutor {
	pub fn new() -> Result<Self, ToolError> {
		let client = Client::builder()
			.timeout(Duration::from_secs(120))
			.build()
			.map_err(|e| ToolError::Internal(format!("Failed to create HTTP client: {e}")))?;

		Ok(Self { client })
	}

	/// Process an image URL, uploading data URIs to CDN if database is available
	async fn process_image_url(&self, url: String, ctx: &ToolContext) -> String {
		if !url.starts_with("data:") {
			return url;
		}

		let Some(db) = &ctx.db else {
			return url;
		};

		match store_from_data_uri(db, &url, ctx.user_id, Some("imagegen")).await {
			Ok(stored) => image_url(stored.id),
			Err(e) => {
				eprintln!("[IMAGEGEN] Failed to upload image to CDN: {}", e);
				url // Return original on failure
			}
		}
	}

	async fn generate_openai(&self, input: &GenerateImageInput, api_key: &str, model: &str) -> Result<Value, ToolError> {
		let payload = json!({
			"model": model,
			"prompt": input.prompt,
			"n": 1,
			"moderation": "low",
			"quality": "low" //TODO: Remove later
		});

		let response = self
			.client
			.post(OPENAI_GENERATIONS_URL)
			.header("Authorization", format!("Bearer {api_key}"))
			.header("Content-Type", "application/json")
			.json(&payload)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("OpenAI API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("OpenAI API error {status}: {error_body}")));
		}

		let openai_response: OpenAIImageResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse OpenAI response: {e}")))?;

		let image_url = openai_response
			.data
			.first()
			.and_then(|d| d.url.clone().or_else(|| d.b64_json.clone().map(|b| format!("data:image/png;base64,{b}"))))
			.ok_or_else(|| ToolError::ExecutionFailed("No image URL in response".to_string()))?;

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn edit_openai(&self, input: &EditImageInput, api_key: &str, model: &str, ctx: &ToolContext) -> Result<Value, ToolError> {
		let image_bytes = self.download_image(&input.image_url, ctx).await?;

		let form = reqwest::multipart::Form::new()
			.part(
				"image",
				reqwest::multipart::Part::bytes(image_bytes).file_name("image.png").mime_str("image/png").unwrap(),
			)
			.text("prompt", input.prompt.clone())
			.text("model", model.to_string())
			.text("n", "1");

		let response = self
			.client
			.post(OPENAI_EDITS_URL)
			.header("Authorization", format!("Bearer {api_key}"))
			.multipart(form)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("OpenAI API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("OpenAI API error {status}: {error_body}")));
		}

		let openai_response: OpenAIImageResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse OpenAI response: {e}")))?;

		let image_url = openai_response
			.data
			.first()
			.and_then(|d| d.url.clone().or_else(|| d.b64_json.clone().map(|b| format!("data:image/png;base64,{b}"))))
			.ok_or_else(|| ToolError::ExecutionFailed("No image URL in response".to_string()))?;

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn generate_replicate(&self, input: &GenerateImageInput, api_key: &str, model: &str) -> Result<Value, ToolError> {
		let url = format!("{REPLICATE_API_BASE}/{model}/predictions");

		let payload = json!({
			"input": {
				"prompt": input.prompt
			}
		});

		let response = self
			.client
			.post(&url)
			.header("Authorization", format!("Bearer {api_key}"))
			.header("Content-Type", "application/json")
			.header("Prefer", "wait=60")
			.json(&payload)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Replicate API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Replicate API error {status}: {error_body}")));
		}

		let replicate_response: ReplicateResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Replicate response: {e}")))?;

		if let Some(error) = replicate_response.error {
			return Err(ToolError::ExecutionFailed(error));
		}

		let image_url = match replicate_response.output {
			Some(Value::Array(arr)) => arr.first().and_then(|v| v.as_str()).map(String::from),
			Some(Value::String(s)) => Some(s),
			_ => None,
		};

		let image_url = image_url.ok_or_else(|| ToolError::ExecutionFailed("No image URL in Replicate response".to_string()))?;

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn edit_replicate(&self, input: &EditImageInput, api_key: &str, model: &str, ctx: &ToolContext) -> Result<Value, ToolError> {
		let url = format!("{REPLICATE_API_BASE}/{model}/predictions");

		let image_bytes = self.download_image(&input.image_url, ctx).await?;
		let image_b64 = BASE64.encode(&image_bytes);
		let image_data_uri = format!("data:image/png;base64,{image_b64}");

		let image_param = get_replicate_image_param(model);

		let payload = json!({
			"input": {
				"prompt": input.prompt,
				image_param: image_data_uri
			}
		});

		let response = self
			.client
			.post(&url)
			.header("Authorization", format!("Bearer {api_key}"))
			.header("Content-Type", "application/json")
			.header("Prefer", "wait=60")
			.json(&payload)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Replicate API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Replicate API error {status}: {error_body}")));
		}

		let replicate_response: ReplicateResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Replicate response: {e}")))?;

		if let Some(error) = replicate_response.error {
			return Err(ToolError::ExecutionFailed(error));
		}

		let image_url = match replicate_response.output {
			Some(Value::Array(arr)) => arr.first().and_then(|v| v.as_str()).map(String::from),
			Some(Value::String(s)) => Some(s),
			_ => None,
		};

		let image_url = image_url.ok_or_else(|| ToolError::ExecutionFailed("No image URL in Replicate response".to_string()))?;

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn generate_google(&self, input: &GenerateImageInput, api_key: &str, model: &str) -> Result<Value, ToolError> {
		let payload = json!({
			"prompt": input.prompt,
			"model": model,
			"response_format": "b64_json",
			"n": 1
		});

		let url = format!("{GOOGLE_GEMINI_GENERATIONS_URL}?key={api_key}");

		let response = self
			.client
			.post(&url)
			.header("Content-Type", "application/json")
			.json(&payload)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Google API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Google API error {status}: {error_body}")));
		}

		let google_response: OpenAIImageResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Google response: {e}")))?;

		let image_url = google_response
			.data
			.first()
			.and_then(|d| d.url.clone().or_else(|| d.b64_json.clone().map(|b| format!("data:image/png;base64,{b}"))))
			.ok_or_else(|| ToolError::ExecutionFailed("No image data in response".to_string()))?;

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn edit_google(&self, input: &EditImageInput, api_key: &str, model: &str, ctx: &ToolContext) -> Result<Value, ToolError> {
		let url = format!("{GOOGLE_GEMINI_CONTENT_URL}/{model}:generateContent");

		let image_bytes = self.download_image(&input.image_url, ctx).await?;
		let image_b64 = BASE64.encode(&image_bytes);

		let payload = json!({
			"contents": [{
				"parts": [
					{ "text": input.prompt },
					{
						"inline_data": {
							"mime_type": "image/png",
							"data": image_b64
						}
					}
				]
			}]
		});

		let response = self
			.client
			.post(&url)
			.header("x-goog-api-key", api_key)
			.header("Content-Type", "application/json")
			.json(&payload)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Google API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Google API error {status}: {error_body}")));
		}

		let gemini_response: GeminiContentResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Google response: {e}")))?;

		let candidates = gemini_response
			.candidates
			.ok_or_else(|| ToolError::ExecutionFailed("No candidates in response".to_string()))?;

		let candidate = candidates
			.first()
			.ok_or_else(|| ToolError::ExecutionFailed("No candidates in response".to_string()))?;

		if candidate.content.is_none() {
			let finish_reason = candidate.finish_reason.as_deref().unwrap_or("UNKNOWN");
			return Err(ToolError::ExecutionFailed(match finish_reason {
				"SAFETY" => "Image generation blocked by safety filters".to_string(),
				"RECITATION" => "Image generation blocked due to potential copyright issues".to_string(),
				"PROHIBITED_CONTENT" => "Image generation blocked due to prohibited content".to_string(),
				_ => format!("Image generation failed: {finish_reason}"),
			}));
		}

		let content = candidate.content.as_ref().unwrap();
		let image_data = content
			.parts
			.iter()
			.find_map(|p| p.inline_data.as_ref())
			.ok_or_else(|| ToolError::ExecutionFailed("No image data in response".to_string()))?;

		let image_url = format!("data:{};base64,{}", image_data.mime_type, image_data.data);

		Ok(json!(ImageResult {
			success: true,
			image_url: Some(image_url),
			error: None,
		}))
	}

	async fn download_image(&self, url: &str, ctx: &ToolContext) -> Result<Vec<u8>, ToolError> {
		if url.starts_with("data:") {
			let parts: Vec<&str> = url.splitn(2, ",").collect();
			if parts.len() != 2 {
				return Err(ToolError::InvalidInput("Invalid data URL format".to_string()));
			}
			return BASE64.decode(parts[1]).map_err(|e| ToolError::InvalidInput(format!("Failed to decode base64: {e}")));
		}

		if let Some(id_str) = url.strip_prefix("/api/images/") {
			let db = ctx.db.as_ref().ok_or_else(|| ToolError::Internal("Database not available".to_string()))?;
			let id = uuid::Uuid::parse_str(id_str).map_err(|e| ToolError::InvalidInput(format!("Invalid image ID: {e}")))?;
			let (data, _mime) = crate::utils::images::get_image(db, id)
				.await
				.map_err(|e| ToolError::Internal(format!("Failed to fetch image: {e}")))?
				.ok_or_else(|| ToolError::InvalidInput("Image not found".to_string()))?;
			return Ok(data);
		}

		let response = self
			.client
			.get(url)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Failed to download image: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			return Err(ToolError::HttpError(format!("Failed to download image: HTTP {status}")));
		}

		response
			.bytes()
			.await
			.map(|b| b.to_vec())
			.map_err(|e| ToolError::HttpError(format!("Failed to read image bytes: {e}")))
	}
}

fn get_replicate_image_param(model: &str) -> &'static str {
	let model_lower = model.to_lowercase();
	if model_lower.contains("nano-banana") {
		"image_input"
	} else if model_lower.contains("flux-redux") {
		"redux_image"
	} else if model_lower.contains("flux-kontext") {
		"input_image"
	} else {
		"image"
	}
}

#[async_trait]
impl ToolExecutor for ImageGenExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		let function = match ctx.function_name.as_deref() {
			Some("generate") | Some("imagegen") | None => "generate",
			Some("edit") => "edit",
			Some(other) => return Err(ToolError::ExecutionFailed(format!("Unknown function: {other}"))),
		};

		let api_key = ctx
			.settings
			.get("api_key")
			.and_then(Value::as_str)
			.ok_or_else(|| ToolError::MissingSetting("api_key".to_string()))?;

		let provider = ctx.settings.get("provider").and_then(Value::as_str).unwrap_or("openai");

		let model = ctx.settings.get("model").and_then(Value::as_str).unwrap_or_else(|| match provider {
			"openai" => "dall-e-3",
			"google" => "imagen-3.0-generate-002",
			_ => "black-forest-labs/flux-schnell",
		});

		let result = match (function, provider) {
			("generate", "openai") => {
				let gen_input: GenerateImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.generate_openai(&gen_input, api_key, model).await?
			}
			("generate", "replicate") => {
				let gen_input: GenerateImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.generate_replicate(&gen_input, api_key, model).await?
			}
			("generate", "google") => {
				let gen_input: GenerateImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.generate_google(&gen_input, api_key, model).await?
			}
			("edit", "openai") => {
				let edit_input: EditImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.edit_openai(&edit_input, api_key, model, ctx).await?
			}
			("edit", "replicate") => {
				let edit_input: EditImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.edit_replicate(&edit_input, api_key, model, ctx).await?
			}
			("edit", "google") => {
				let edit_input: EditImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.edit_google(&edit_input, api_key, model, ctx).await?
			}
			_ => {
				let gen_input: GenerateImageInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.generate_openai(&gen_input, api_key, model).await?
			}
		};

		// Process image URLs in the result - upload data URIs to CDN
		if let Some(image_url_val) = result.get("image_url").and_then(|v| v.as_str()) {
			let processed_url = self.process_image_url(image_url_val.to_string(), ctx).await;
			let mut result_obj = result.as_object().unwrap().clone();
			result_obj.insert("image_url".to_string(), Value::String(processed_url));
			result_obj.insert(
				"message".to_string(),
				Value::String(
					"Image was successfully generated and uploaded to the CDN, you can show the image by using the following markdown: ![]({processed_url}). No other action is needed.".to_string(),
				),
			);
			return Ok(Value::Object(result_obj));
		}

		Ok(result)
	}

	fn name(&self) -> &str {
		"imagegen"
	}
}
