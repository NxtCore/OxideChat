//! Admin i18n endpoints.
//!
//! Provides endpoints for managing translations.

use crate::i18n::I18n;
use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::consts::{ADMIN_I18N_EDIT, ADMIN_I18N_VIEW};
use crate::types::{AppError, AuthUser, JobState, Translation, TranslationsResponse, UpsertTranslationRequest, json_ok};
use crate::utils::response::ErrorCode;
use axum::{
	Json,
	extract::{Path, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/i18n
///
/// List all translations.
pub async fn list_translations(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_I18N_VIEW).await?;

	let translations = Translation::list_all(&state.db).await.map_err(AppError::database)?;
	json_ok(TranslationsResponse { translations })
}

/// PUT /api/v1/admin/i18n/translations
///
/// Create or update a translation. Reloads translations after change.
pub async fn upsert_translation(State(state): State<Arc<JobState>>, auth: AuthUser, Json(payload): Json<UpsertTranslationRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_I18N_EDIT).await?;

	let id = Translation::upsert(&state.db, &payload.language, &payload.key_path, &payload.value, payload.is_override)
		.await
		.map_err(AppError::database)?;
	I18n::get().reload(&state.db).await;
	AuditLog::log(&state.db, LogEvent::TranslationUpdated, Some(auth.id), Some(EntityType::Translation), Some(id));
	json_ok(serde_json::json!({ "id": id, "success": true }))
}

/// DELETE /api/v1/admin/i18n/translations/:id
///
/// Delete a translation by ID. Reloads translations after change.
pub async fn delete_translation(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_I18N_EDIT).await?;

	let deleted = Translation::delete(&state.db, &id)
		.await
		.map_err(AppError::database)?;
	if !deleted {
		return Err(ErrorCode::TranslationNotFound.into());
	}
	I18n::get().reload(&state.db).await;
	AuditLog::log(&state.db, LogEvent::TranslationDeleted, Some(auth.id), Some(EntityType::Translation), Some(id));
	json_ok(serde_json::json!({ "success": true }))
}
