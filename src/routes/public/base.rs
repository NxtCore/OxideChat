//! Base endpoint handler.
//!
//! Returns application base data including translations.

use crate::i18n::I18n;
use crate::types::{AppError, BaseResponse, JobState, RequestContext, Role, User, json_ok};
use crate::utils::response::{ResponseBody, ResponseBuilder};
use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

/// GET /api/v1/health
///
/// Liveness probe — responds the moment the TCP listener binds, no DB access.
pub async fn health() -> impl IntoResponse {
	ResponseBuilder::new(ResponseBody::Json(serde_json::json!({"status": "ok"}))).build()
}

/// GET /api/v1/base
///
/// Returns base application data including translations and setup status. Works without a session.
///
/// # Errors
///
/// Returns 500 if the database query fails.
pub async fn get_base(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>) -> Result<Response, AppError> {
	let needs_setup = !User::any_exist(&state.db).await.map_err(AppError::database)?;

	let roles = match user {
		Some(_) => Role::list_all(&state.db).await.unwrap_or_else(|e| {
			tracing::warn!(error = %e, "failed to list roles");
			vec![]
		}),
		None => vec![],
	};

	let config = crate::config::Config::get();
	json_ok(BaseResponse {
		i18n: I18n::get().all(),
		language: config.language().as_str().to_string(),
		needs_setup,
		oauth_providers: config.get_configured_oauth_providers(),
		roles,
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	})
}
