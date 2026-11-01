//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::config::Config;
use crate::types::consts::ADMIN_CONFIG_EDIT;
use crate::types::{AppError, AuthUser, GlobalConfigResponse, JobState, UpdateGlobalConfigRequest, json_ok};
use crate::utils::response::ErrorCode;
use axum::{Json, extract::State, response::Response};
use std::sync::Arc;

/// GET /api/v1/config
///
/// Get global configuration (requires an authenticated session).
pub async fn get_global_config(_: AuthUser) -> Result<Response, AppError> {
	let config = Config::get();
	json_ok(GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	})
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only).
pub async fn update_global_config(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<UpdateGlobalConfigRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;

	if let Some(default_theme) = req.default_theme {
		Config::get()
			.set_default_theme(&state.db, &default_theme)
			.await
			.inspect_err(|e| tracing::error!("Failed to update default_theme: {e}"))
			.map_err(|_| ErrorCode::InternalError)?;
	}

	if let Some(enable_provider_selector) = req.enable_provider_selector {
		Config::get().set_enable_provider_selector(&state.db, enable_provider_selector).await?;
	}

	if let Some(allow_server_stdio_mcp) = req.allow_server_stdio_mcp {
		Config::get().set_allow_server_stdio_mcp(&state.db, allow_server_stdio_mcp).await?;
	}

	if let Some(default_model_id) = req.default_model_id {
		Config::get().set_default_model_id(&state.db, default_model_id).await?;
	}

	let config = Config::get();
	json_ok(GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	})
}
