//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::config::Config;
use crate::logging::{AuditLog, LogEvent};
use crate::types::consts::{ADMIN_CONFIG_EDIT, ADMIN_CONFIG_VIEW};
use crate::types::{AdminConfigResponse, AppError, AuthUser, GlobalConfigResponse, JobState, UpdateGlobalConfigRequest, json_ok};
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

/// GET /api/v1/admin/config
///
/// Settings only admins may read. Secrets are never returned.
pub async fn get_admin_config(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_VIEW).await?;
	json_ok(AdminConfigResponse {
		oauth: Config::get().oauth_settings(),
	})
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only).
pub async fn update_global_config(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<UpdateGlobalConfigRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;

	Config::get().apply_update(&state.db, &req).await?;
	AuditLog::log(&state.db, LogEvent::ConfigUpdated, Some(auth.id), None, None);

	let config = Config::get();
	json_ok(GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	})
}
