use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{PreferencesResponse, UpdatePreferencesRequest, UserPreferences};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::State, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

/// GET /api/v1/users/@me/preferences
pub async fn get_preferences(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match UserPreferences::find_by_user_id(&state.db, &user.id).await {
		Ok(Some(prefs)) => ResponseBuilder::new(ResponseBody::Json(PreferencesResponse::from(prefs))).build(),
		Ok(None) => ResponseBuilder::new(ResponseBody::Json(PreferencesResponse::default())).build(),
		Err(e) => {
			eprintln!("[PREFERENCES] Failed to get preferences: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/users/@me/preferences
pub async fn update_preferences(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<UpdatePreferencesRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if let Some(ref animation) = req.streaming_animation {
		if !matches!(animation.to_lowercase().as_str(), "fade" | "typewriter" | "slide" | "none") {
			return ErrorBuilder::new(ErrorCode::BadRequest).build();
		}
	}

	match UserPreferences::upsert(&state.db, &user.id, &req).await {
		Ok(prefs) => ResponseBuilder::new(ResponseBody::Json(PreferencesResponse::from(prefs))).build(),
		Err(e) => {
			eprintln!("[PREFERENCES] Failed to update preferences: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
