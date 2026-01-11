//! Global configuration admin routes.
//!
//! Admin endpoints for managing instance-wide settings like default theme.

use crate::AppState;
use crate::types::{GlobalConfig, GlobalConfigResponse, ThemeCssVars, UpdateGlobalConfigRequest};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::State, response::IntoResponse};
use std::sync::Arc;

/// GET /api/v1/config
///
/// Get global configuration (public endpoint).
pub async fn get_global_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
	let config = sqlx::query_as::<_, GlobalConfig>("SELECT * FROM global_config WHERE key = 'default_theme'")
		.fetch_optional(&state.db)
		.await;

	match config {
		Ok(Some(config)) => {
			let default_theme: ThemeCssVars = serde_json::from_value(config.value).unwrap_or_default();
			let response = GlobalConfigResponse { default_theme };
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => {
			let response = GlobalConfigResponse::default();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[CONFIG] Failed to get global config: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/admin/config
///
/// Update global configuration (admin only - protected by route).
pub async fn update_global_config(
	State(state): State<Arc<AppState>>,
	Json(req): Json<UpdateGlobalConfigRequest>,
) -> impl IntoResponse {
	if let Some(default_theme) = req.default_theme {
		let theme_value = serde_json::to_value(&default_theme).unwrap_or_default();

		let result = sqlx::query(
			r#"
			INSERT INTO global_config (key, value, updated_at)
			VALUES ('default_theme', $1, NOW())
			ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()
			"#,
		)
		.bind(&theme_value)
		.execute(&state.db)
		.await;

		if let Err(e) = result {
			eprintln!("[CONFIG] Failed to update global config: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	let config = sqlx::query_as::<_, GlobalConfig>("SELECT * FROM global_config WHERE key = 'default_theme'")
		.fetch_optional(&state.db)
		.await;

	match config {
		Ok(Some(config)) => {
			let default_theme: ThemeCssVars = serde_json::from_value(config.value).unwrap_or_default();
			let response = GlobalConfigResponse { default_theme };
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => {
			let response = GlobalConfigResponse::default();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[CONFIG] Failed to get global config after update: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
