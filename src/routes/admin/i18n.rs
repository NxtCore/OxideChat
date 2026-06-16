//! Admin i18n endpoints.
//!
//! Provides endpoints for managing translations.

use crate::i18n::I18n;
use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{Translation, TranslationsResponse, UpsertTranslationRequest};

use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;

pub const ADMIN_I18N_VIEW: &str = "admin.i18n.view";
pub const ADMIN_I18N_EDIT: &str = "admin.i18n.edit";

/// GET /api/v1/admin/i18n
///
/// List all translations.
pub async fn list_translations(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_I18N_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let translations = Translation::list_all(&state.db).await.unwrap_or_default();

	ResponseBuilder::new(ResponseBody::Json(TranslationsResponse { translations })).build()
}

/// PUT /api/v1/admin/i18n/translations
///
/// Create or update a translation. Reloads translations after change.
pub async fn upsert_translation(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<UpsertTranslationRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_I18N_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let result = Translation::upsert(&state.db, &payload.language, &payload.key_path, &payload.value, payload.is_override).await;

	match result {
		Ok(id) => {
			I18n::get().reload(&state.db).await;
			AuditLog::log(&state.db, LogEvent::TranslationUpdated, None, Some(EntityType::Translation), Some(id));
			ResponseBuilder::new(ResponseBody::Json(serde_json::json!({ "id": id, "success": true }))).build()
		}
		Err(e) => {
			eprintln!("[I18N] Failed to upsert translation: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// DELETE /api/v1/admin/i18n/translations/:id
///
/// Delete a translation by ID. Reloads translations after change.
pub async fn delete_translation(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<sqlx::types::Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_I18N_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let result = Translation::delete(&state.db, &id).await;

	match result {
		Ok(true) => {
			I18n::get().reload(&state.db).await;
			AuditLog::log(&state.db, LogEvent::TranslationDeleted, None, Some(EntityType::Translation), Some(id));
			ResponseBuilder::new(ResponseBody::Json(serde_json::json!({ "success": true }))).build()
		}
		Ok(false) => ErrorBuilder::new(ErrorCode::TranslationNotFound).build(),
		Err(e) => {
			eprintln!("[I18N] Failed to delete translation: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
