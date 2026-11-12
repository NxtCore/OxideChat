//! Preset routes: personal presets, team presets managed by team admins, and installation presets.

use crate::types::consts::{ADMIN_PRESETS_EDIT, SETTINGS_PRESETS_EDIT};
use crate::types::{
	AppError, AuthUser, Budget, ReplaceAssignmentsRequest, BudgetSubject, JobState, Preset, PresetActor, PresetError, PresetResponse, PresetSlugAvailability, PresetSlugQuery, SavePresetRequest,
	is_valid_preset_slug, json_ok,
};
use crate::utils::response::{ResponseBody, ResponseBuilder};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

async fn actor(state: &JobState, user: &AuthUser) -> Result<PresetActor, AppError> {
	let personal = user.check_permission(&state.db, SETTINGS_PRESETS_EDIT).await?;
	let installation_admin = user.check_permission(&state.db, ADMIN_PRESETS_EDIT).await?;
	Ok(PresetActor::load(&state.db, &user.id, personal, installation_admin).await?)
}

async fn responses(state: &JobState, actor: &PresetActor, presets: Vec<Preset>) -> Result<Vec<PresetResponse>, AppError> {
	let mut responses = Preset::responses_for_user(&state.db, actor, presets).await?;
	let blocked: HashSet<Uuid> = match Budget::status_for(&state.db, BudgetSubject::user(actor.user_id)).await {
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

async fn single(state: &JobState, actor: &PresetActor, preset: Preset) -> Result<PresetResponse, AppError> {
	responses(state, actor, vec![preset]).await?.pop().ok_or_else(|| PresetError::NotFound.into())
}

/// GET /api/v1/presets
///
/// Every preset the user can see, with its base model and whether it can be used.
pub async fn list_presets(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	let presets = Preset::list_for_user(&state.db, &user.id).await?;
	json_ok(responses(&state, &actor, presets).await?)
}

/// GET /api/v1/presets/scopes
///
/// The scopes and teams the user may create presets in.
pub async fn list_scopes(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	json_ok(actor.scopes(&state.db).await?)
}

/// GET /api/v1/presets/{id}
pub async fn get_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	let preset = Preset::find_for_actor(&state.db, &actor, &id).await?.ok_or(PresetError::NotFound)?;
	json_ok(single(&state, &actor, preset).await?)
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
	let actor = actor(&state, &user).await?;
	let preset = Preset::create(&state.db, &actor, request).await?;
	let response = single(&state, &actor, preset).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// PUT /api/v1/presets/{id}
pub async fn update_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SavePresetRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	let preset = Preset::update(&state.db, &actor, &id, request).await?;
	json_ok(single(&state, &actor, preset).await?)
}

/// DELETE /api/v1/presets/{id}
pub async fn delete_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	Preset::delete(&state.db, &actor, &id).await?;
	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).status(StatusCode::NO_CONTENT).build())
}

/// GET /api/v1/presets/{id}/assignments
///
/// Who can use the preset: its owner and everyone, the teams and the users it is shared with.
pub async fn get_preset_assignments(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	let preset = Preset::find_for_actor(&state.db, &actor, &id).await?.ok_or(PresetError::NotFound)?;
	json_ok(preset.assignments(&state.db, &actor).await?)
}

/// PUT /api/v1/presets/{id}/assignments
///
/// Makes the targets in the body the only ones the preset is shared with; its owner always keeps it.
pub async fn replace_preset_assignments(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<ReplaceAssignmentsRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &user).await?;
	let preset = Preset::replace_assignments(&state.db, &actor, &id, &request).await?;
	json_ok(preset.assignments(&state.db, &actor).await?)
}
