//! Models routes.
//!
//! Public endpoint for listing available AI models.

use crate::AppState;
use crate::ai;
use crate::routes::public::auth::get_current_user;
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
	pub stable_key: String,
	pub capabilities: Vec<String>,
	pub context_length: Option<u32>,
	pub max_tokens: Option<u32>,
	pub provider_name: String,
	pub provider_kind: String,
	pub is_favorite: bool,
	pub is_hidden: bool,
}

/// GET /api/v1/models
///
/// List all available AI models from registered providers.
pub async fn list_models(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	// Authenticate user
	let _user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let engine = ai::get();
	let engine_read = engine.read().await;
	let models = engine_read.list_models().await;
	drop(engine_read);

	// Convert to response format
	let responses: Vec<ModelResponse> = models
		.into_iter()
		.map(|m| {
			let provider_kind_str = match &m.provider_kind {
				omniference::types::ProviderKind::OpenAI => "OPENAI",
				omniference::types::ProviderKind::OpenAICompat => "OPENAI_COMPAT",
				omniference::types::ProviderKind::OpenRouter => "OPENROUTER",
				omniference::types::ProviderKind::Anthropic => "ANTHROPIC",
				omniference::types::ProviderKind::Google => "GOOGLE",
				omniference::types::ProviderKind::Custom(s) => "CUSTOM",
			};

			ModelResponse {
				id: m.id.clone(),
				provider_id: m.provider_name.clone(),
				model_id: m.id.clone(),
				display_name: m.name.clone(),
				stable_key: m.id.clone(),
				capabilities: m.capabilities.iter().map(|c| c.as_str().to_string()).collect(),
				context_length: m.context_length,
				max_tokens: m.max_tokens,
				provider_name: m.provider_name.clone(),
				provider_kind: provider_kind_str.to_string(),
				is_favorite: false,
				is_hidden: false,
			}
		})
		.collect();

	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}
