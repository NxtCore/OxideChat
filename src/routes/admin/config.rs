//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::config::Config;
use crate::types::JobState;
use crate::types::{GlobalConfigResponse, RequestContext, UpdateGlobalConfigRequest};

use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::{Extension, State}, response::IntoResponse};
use std::sync::Arc;

pub const ADMIN_CONFIG_EDIT: &str = "admin.config.edit";

/// GET /api/v1/config
///
/// Get global configuration (public endpoint).
pub async fn get_global_config() -> impl IntoResponse {
	let config = Config::get();
	let response = GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only).
pub async fn update_global_config(State(state): State<Arc<JobState>>, Extension(RequestContext { user: current_user }): Extension<RequestContext>, Json(req): Json<UpdateGlobalConfigRequest>) -> impl IntoResponse {
	let user = match current_user {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_CONFIG_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if let Some(default_theme) = req.default_theme {
		if let Err(e) = Config::get().set_default_theme(&state.db, &default_theme).await {
			eprintln!("[CONFIG] Failed to update default_theme: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Some(enable_provider_selector) = req.enable_provider_selector {
		if let Err(e) = Config::get().set_enable_provider_selector(&state.db, enable_provider_selector).await {
			eprintln!("[CONFIG] Failed to update enable_provider_selector: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Some(allow_server_stdio_mcp) = req.allow_server_stdio_mcp {
		if let Err(e) = Config::get().set_allow_server_stdio_mcp(&state.db, allow_server_stdio_mcp).await {
			eprintln!("[CONFIG] Failed to update allow_server_stdio_mcp: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}


	if let Some(default_model_id) = req.default_model_id {
		if let Err(e) = Config::get().set_default_model_id(&state.db, default_model_id).await {
			eprintln!("[CONFIG] Failed to update default_model_id: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	let config = Config::get();
	let response = GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
		allow_server_stdio_mcp: config.allow_server_stdio_mcp(),
		default_model_id: config.default_model_id(),
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}
