//! Preset routes: a user's own presets and the shared presets they can use.

use crate::types::consts::SETTINGS_PRESETS_EDIT;
use crate::types::{
	AppError, AuthUser, Budget, BudgetSubject, JobState, Preset, PresetError, PresetResponse, PresetSlugAvailability, PresetSlugQuery, SavePresetRequest, is_valid_preset_slug, json_ok,
};
use crate::utils::response::{ResponseBody, ResponseBuilder};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

async fn responses(state: &JobState, user: &AuthUser, presets: Vec<Preset>) -> Result<Vec<PresetResponse>, AppError> {
	let mut responses = Preset::responses_for_user(&state.db, &user.id, presets).await?;
	let blocked: HashSet<Uuid> = match Budget::status_for(&state.db, BudgetSubject::user(user.id)).await {
		Ok(status) => status.blocked_model_ids.into_iter().collect(),
		Err(error) => {
			tracing::warn!(%error, "failed to load budget status for presets");
			HashSet::new()
		}
	};
	for base in responses.iter_mut().filter_map(|response| response.base_model.as_mut()) {
		base.budget_blocked = blocked.contains(&base.id);
	}
	Ok(responses)
}

async fn single(state: &JobState, user: &AuthUser, preset: Preset) -> Result<PresetResponse, AppError> {
	responses(state, user, vec![preset]).await?.pop().ok_or_else(|| PresetError::NotFound.into())
}

/// GET /api/v1/presets
///
/// Every preset the user can see, with its base model and whether it can be used.
pub async fn list_presets(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let presets = Preset::list_for_user(&state.db, &user.id).await?;
	json_ok(responses(&state, &user, presets).await?)
}

/// GET /api/v1/presets/{id}
pub async fn get_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let preset = Preset::find_visible(&state.db, &user.id, &id).await?.ok_or(PresetError::NotFound)?;
	json_ok(single(&state, &user, preset).await?)
}

/// GET /api/v1/presets/check-slug
///
/// Whether a slug is well formed and not used by another preset.
pub async fn check_slug(State(state): State<Arc<JobState>>, _user: AuthUser, Query(query): Query<PresetSlugQuery>) -> Result<Response, AppError> {
	let slug = query.slug.trim();
	let valid = is_valid_preset_slug(slug);
	let available = valid && Preset::slug_available(&state.db, slug, query.exclude_id.as_ref()).await?;
	json_ok(PresetSlugAvailability { valid, available })
}

/// POST /api/v1/presets
pub async fn create_preset(State(state): State<Arc<JobState>>, user: AuthUser, Json(request): Json<SavePresetRequest>) -> Result<Response, AppError> {
	user.require(&state.db, SETTINGS_PRESETS_EDIT).await?;
	let preset = Preset::create_personal(&state.db, &user.id, request).await?;
	let response = single(&state, &user, preset).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// PUT /api/v1/presets/{id}
pub async fn update_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SavePresetRequest>) -> Result<Response, AppError> {
	user.require(&state.db, SETTINGS_PRESETS_EDIT).await?;
	let preset = Preset::update_personal(&state.db, &user.id, &id, request).await?;
	json_ok(single(&state, &user, preset).await?)
}

/// DELETE /api/v1/presets/{id}
pub async fn delete_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	user.require(&state.db, SETTINGS_PRESETS_EDIT).await?;
	Preset::delete_personal(&state.db, &user.id, &id).await?;
	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).status(StatusCode::NO_CONTENT).build())
}
