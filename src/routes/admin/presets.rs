//! Installation-wide preset administration: list every preset, rename it or turn it on or off.

use crate::types::consts::{ADMIN_PRESETS_EDIT, ADMIN_PRESETS_VIEW};
use crate::types::{AdminPresetQuery, AdminUpdatePresetRequest, AppError, AuthUser, JobState, Preset, json_ok};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/presets
pub async fn list_presets(State(state): State<Arc<JobState>>, user: AuthUser, Query(query): Query<AdminPresetQuery>) -> Result<Response, AppError> {
	user.require(&state.db, ADMIN_PRESETS_VIEW).await?;
	json_ok(Preset::admin_list(&state.db, &query).await?)
}

/// PATCH /api/v1/admin/presets/{id}
pub async fn update_preset(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<AdminUpdatePresetRequest>) -> Result<Response, AppError> {
	user.require(&state.db, ADMIN_PRESETS_EDIT).await?;
	json_ok(Preset::admin_update(&state.db, &user.id, &id, request).await?)
}
