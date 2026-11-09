//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::config::Config;
use crate::logging::{AuditLog, LogEvent};
use crate::types::consts::ADMIN_CONFIG_EDIT;
use crate::utils::images::is_safe_stored_image;
use crate::utils::response::ErrorCode;
use crate::types::{AppError, AuthUser, GlobalConfigResponse, JobState, MAX_INSTANCE_NAME_CHARS, UpdateGlobalConfigRequest, json_ok};
use axum::{Json, extract::State, response::Response};
use std::sync::Arc;

/// GET /api/v1/config
///
/// Get global configuration (requires an authenticated session).
pub async fn get_global_config(_: AuthUser) -> Result<Response, AppError> {
	let config = Config::get();
	json_ok(GlobalConfigResponse {
		default_theme: config.default_theme(),
		provider_routing: config.provider_routing(),
		default_model_id: config.default_model_id(),
	})
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only).
pub async fn update_global_config(State(state): State<Arc<JobState>>, auth: AuthUser, Json(mut req): Json<UpdateGlobalConfigRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;
	let instance = req.instance.as_ref();
	if instance.and_then(|i| i.name.as_ref()).and_then(Option::as_deref).is_some_and(|name| name.trim().chars().count() > MAX_INSTANCE_NAME_CHARS) {
		return Err(ErrorCode::BadRequest.into());
	}
	if let Some(Some(icon_id)) = instance.and_then(|i| i.icon_id)
		&& !is_safe_stored_image(&state.db, icon_id).await.map_err(|e| {
			tracing::error!(image_id = %icon_id, error = %e, "failed to read instance icon");
			ErrorCode::InternalError
		})? {
		return Err(ErrorCode::BadRequest.into());
	}

	if let Some(routing) = &req.provider_routing {
		req.provider_routing = Some(routing.normalized().ok_or(ErrorCode::ValidationFailed)?);
	}

	Config::get().apply_update(&state.db, &req).await?;
	AuditLog::log(&state.db, LogEvent::ConfigUpdated, Some(auth.id), None, None);

	let config = Config::get();
	json_ok(GlobalConfigResponse {
		default_theme: config.default_theme(),
		provider_routing: config.provider_routing(),
		default_model_id: config.default_model_id(),
	})
}
