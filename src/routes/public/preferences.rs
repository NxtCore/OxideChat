//! User preferences routes.
//!
//! Get and update user preferences for the chat interface.

use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{PreferencesResponse, UpdatePreferencesRequest, UserPreferences};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::State, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

/// GET /api/v1/users/@me/preferences
///
/// Get user preferences. Returns defaults if no preferences exist.
pub async fn get_preferences(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let prefs = sqlx::query_as::<_, UserPreferences>("SELECT * FROM user_preferences WHERE user_id = $1")
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	match prefs {
		Ok(Some(prefs)) => {
			let response: PreferencesResponse = prefs.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => {
			let response = PreferencesResponse::default();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[PREFERENCES] Failed to get preferences: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/users/@me/preferences
///
/// Update user preferences. Creates if doesn't exist (upsert).
pub async fn update_preferences(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<UpdatePreferencesRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if let Some(ref animation) = req.streaming_animation {
		let valid = matches!(animation.to_lowercase().as_str(), "fade" | "typewriter" | "slide" | "none");
		if !valid {
			return ErrorBuilder::new(ErrorCode::BadRequest).build();
		}
	}

	let existing = sqlx::query_as::<_, UserPreferences>("SELECT * FROM user_preferences WHERE user_id = $1")
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let result = match existing {
		Ok(Some(existing)) => {
			let default_model_key = req.default_model_key.or(existing.default_model_key);
			let favorite_model_keys = req
				.favorite_model_keys
				.map(|keys| serde_json::to_value(keys).unwrap_or_default())
				.unwrap_or(existing.favorite_model_keys);
			let streaming_animation = req.streaming_animation.unwrap_or(existing.streaming_animation);
			let use_remend = req.use_remend.unwrap_or(existing.use_remend);
			let theme_css_vars = req
				.theme_css_vars
				.map(|v| serde_json::to_value(v).unwrap_or_default())
				.unwrap_or(existing.theme_css_vars);
			let custom_theme_urls = req
				.custom_theme_urls
				.map(|urls| serde_json::to_value(urls).unwrap_or_default())
				.unwrap_or(existing.custom_theme_urls);

			sqlx::query_as::<_, UserPreferences>(
				r#"
				UPDATE user_preferences
				SET default_model_key = $2, favorite_model_keys = $3, 
					streaming_animation = $4, use_remend = $5,
					theme_css_vars = $6, custom_theme_urls = $7,
					updated_at = NOW()
				WHERE user_id = $1
				RETURNING *
				"#,
			)
			.bind(user.id)
			.bind(&default_model_key)
			.bind(&favorite_model_keys)
			.bind(&streaming_animation)
			.bind(use_remend)
			.bind(&theme_css_vars)
			.bind(&custom_theme_urls)
			.fetch_one(&state.db)
			.await
		}
		Ok(None) => {
			let default_model_key = req.default_model_key;
			let favorite_model_keys = serde_json::to_value(req.favorite_model_keys.unwrap_or_default()).unwrap_or_default();
			let streaming_animation = req.streaming_animation.unwrap_or_else(|| "fade".to_string());
			let use_remend = req.use_remend.unwrap_or(true);
			let theme_css_vars = serde_json::to_value(req.theme_css_vars.unwrap_or_default()).unwrap_or_default();
			let custom_theme_urls = serde_json::to_value(req.custom_theme_urls.unwrap_or_default()).unwrap_or_default();

			sqlx::query_as::<_, UserPreferences>(
				r#"
				INSERT INTO user_preferences (user_id, default_model_key, favorite_model_keys, streaming_animation, use_remend, theme_css_vars, custom_theme_urls)
				VALUES ($1, $2, $3, $4, $5, $6, $7)
				RETURNING *
				"#,
			)
			.bind(user.id)
			.bind(&default_model_key)
			.bind(&favorite_model_keys)
			.bind(&streaming_animation)
			.bind(use_remend)
			.bind(&theme_css_vars)
			.bind(&custom_theme_urls)
			.fetch_one(&state.db)
			.await
		}
		Err(e) => {
			eprintln!("[PREFERENCES] Failed to fetch existing preferences: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	match result {
		Ok(prefs) => {
			let response: PreferencesResponse = prefs.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[PREFERENCES] Failed to update preferences: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
