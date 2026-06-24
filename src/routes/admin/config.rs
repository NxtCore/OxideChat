//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::config::Config;
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{GlobalConfigResponse, UpdateGlobalConfigRequest};

use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::State, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

pub const ADMIN_CONFIG_EDIT: &str = "admin.config.edit";

/// GET /api/v1/config
///
/// Get global configuration (public endpoint).
pub async fn get_global_config() -> impl IntoResponse {
	let config = Config::get();
	let response = GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only).
pub async fn update_global_config(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<UpdateGlobalConfigRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_CONFIG_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if let Some(default_theme) = req.default_theme {
		let theme_json = match serde_json::to_string(&default_theme) {
			Ok(json) => json,
			Err(e) => {
				eprintln!("[CONFIG] Failed to serialize theme: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
		};

		let result = sqlx::query(
			r#"
			INSERT INTO app_config (key, value)
			VALUES ('default_theme', $1)
			ON CONFLICT (key) DO UPDATE SET value = $1
			"#,
		)
		.bind(&theme_json)
		.execute(&state.db)
		.await;

		if let Err(e) = result {
			eprintln!("[CONFIG] Failed to update default_theme: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}

		Config::get().reload(&state.db).await;
	}

	if let Some(enable_provider_selector) = req.enable_provider_selector {
		let result = sqlx::query(
			r#"
			INSERT INTO app_config (key, value)
			VALUES ('enable_provider_selector', $1)
			ON CONFLICT (key) DO UPDATE SET value = $1
			"#,
		)
		.bind(if enable_provider_selector { "true" } else { "false" })
		.execute(&state.db)
		.await;

		if let Err(e) = result {
			eprintln!("[CONFIG] Failed to update enable_provider_selector: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}

		Config::get().reload(&state.db).await;
	}

	let config = Config::get();
	let response = GlobalConfigResponse {
		default_theme: config.default_theme(),
		enable_provider_selector: config.enable_provider_selector(),
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}
