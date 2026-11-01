use crate::types::{AppError, AuthUser, JobState, PreferencesResponse, Team, UpdatePreferencesRequest, UserPreferences, json_ok};
use crate::utils::response::ErrorCode;
use axum::{Json, extract::State, response::Response};
use std::sync::Arc;

/// GET /api/v1/users/@me/preferences
pub async fn get_preferences(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let response = match UserPreferences::find_by_user_id(&state.db, &user.id).await? {
		Some(prefs) => {
			let default_model_key = prefs.default_model_key.clone();
			let mut response = PreferencesResponse::from(prefs);
			response.effective_default_model_key = Team::resolve_default_model_key(&state.db, &user.id, default_model_key).await;
			response
		}
		None => PreferencesResponse { effective_default_model_key: Team::resolve_default_model_key(&state.db, &user.id, None).await, ..Default::default() },
	};
	json_ok(response)
}

/// PATCH /api/v1/users/@me/preferences
pub async fn update_preferences(State(state): State<Arc<JobState>>, user: AuthUser, Json(req): Json<UpdatePreferencesRequest>) -> Result<Response, AppError> {
	if let Some(ref animation) = req.streaming_animation
		&& !matches!(animation.to_lowercase().as_str(), "fade" | "typewriter" | "slide" | "none") {
			return Err(ErrorCode::BadRequest.into());
		}

	let prefs = UserPreferences::upsert(&state.db, &user.id, &req).await?;
	let default_model_key = prefs.default_model_key.clone();
	let mut response = PreferencesResponse::from(prefs);
	response.effective_default_model_key = Team::resolve_default_model_key(&state.db, &user.id, default_model_key).await;
	json_ok(response)
}
