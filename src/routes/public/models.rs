//! Models routes.
//!
//! Public endpoint for listing available AI models.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::types::{AiModel, ProviderKind};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use std::sync::Arc;
use tower_cookies::Cookies;

/// Model response for the frontend
#[derive(Debug, Serialize)]
pub struct ModelResponse {
	pub id: String,
	pub provider_id: String,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<u32>,
	pub max_tokens: Option<u32>,
	pub provider_name: String,
	pub provider_kind: String,
}

/// GET /api/v1/models
///
/// List all available AI models from the database.
pub async fn list_models(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	// Authenticate user
	let _user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Fetch models from database instead of Omniference
	let models = match sqlx::query_as::<_, AiModel>(
		"SELECT m.* FROM models m 
		JOIN providers p ON m.provider_id = p.id 
		WHERE p.is_enabled = true AND m.is_enabled = true 
		ORDER BY m.display_name",
	)
	.fetch_all(&state.db)
	.await
	{
		Ok(models) => models,
		Err(e) => {
			eprintln!("[AI] Failed to fetch models from database: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Fetch provider information for all models
	let provider_ids: Vec<_> = models.iter().map(|m| m.provider_id).collect();
	let providers = match sqlx::query_as::<_, (uuid::Uuid, String, ProviderKind)>("SELECT id, name, kind FROM providers WHERE id = ANY($1)")
		.bind(&provider_ids)
		.fetch_all(&state.db)
		.await
	{
		Ok(providers) => providers,
		Err(e) => {
			eprintln!("[AI] Failed to fetch providers: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Create a map for quick lookup
	let provider_map: std::collections::HashMap<uuid::Uuid, (String, ProviderKind)> = providers.into_iter().map(|(id, name, kind)| (id, (name, kind))).collect();

	// Convert to response format
	let responses: Vec<ModelResponse> = models
		.into_iter()
		.filter_map(|m| {
			let (provider_name, provider_kind) = provider_map.get(&m.provider_id)?;
			let provider_kind_str = match provider_kind {
				ProviderKind::Openai => "OPENAI",
				ProviderKind::OpenaiCompat => "OPENAI_COMPAT",
				ProviderKind::Openrouter => "OPENROUTER",
				ProviderKind::Anthropic => "ANTHROPIC",
				ProviderKind::Google => "GOOGLE",
				ProviderKind::Custom => "CUSTOM",
			};

			// Convert capabilities from JSON to string vector
			let capabilities = if let serde_json::Value::Array(arr) = &m.capabilities {
				arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
			} else {
				vec![]
			};
			let input_modalities = if let serde_json::Value::Array(arr) = &m.input_modalities {
				arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
			} else {
				vec![]
			};
			let output_modalities = if let serde_json::Value::Array(arr) = &m.output_modalities {
				arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
			} else {
				vec![]
			};

			Some(ModelResponse {
				id: m.id.to_string(),
				provider_id: m.provider_id.to_string(),
				model_id: m.model_id.clone(),
				display_name: m.display_name.clone(),
				capabilities,
				context_length: m.context_length.map(|c| c as u32),
				max_tokens: m.max_tokens.map(|c| c as u32),
				provider_name: provider_name.clone(),
				provider_kind: provider_kind_str.to_string(),
				input_modalities,
				output_modalities,
			})
		})
		.collect();

	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}
