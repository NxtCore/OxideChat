//! Admin i18n endpoints.
//!
//! Provides endpoints for managing translations.

use crate::i18n::I18n;
use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{IdRow, Translation, TranslationsResponse, UpsertTranslationRequest};

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

	let translations: Vec<Translation> = sqlx::query_as("SELECT id, language, key_path, value, is_override FROM i18n_translations ORDER BY language, key_path")
		.fetch_all(&state.db)
		.await
		.unwrap_or_default();

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

	let result: Result<IdRow, _> = sqlx::query_as(
		r#"
        INSERT INTO i18n_translations (language, key_path, value, is_override)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (language, key_path)
        DO UPDATE SET value = EXCLUDED.value, is_override = EXCLUDED.is_override, updated_at = NOW()
        RETURNING id
        "#,
	)
	.bind(&payload.language)
	.bind(&payload.key_path)
	.bind(&payload.value)
	.bind(payload.is_override)
	.fetch_one(&state.db)
	.await;

	match result {
		Ok(row) => {
			I18n::get().reload(&state.db).await;
			AuditLog::log(&state.db, LogEvent::TranslationUpdated, None, Some(EntityType::Translation), Some(row.id));
			ResponseBuilder::new(ResponseBody::Json(serde_json::json!({ "id": row.id, "success": true }))).build()
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

	let result = sqlx::query("DELETE FROM i18n_translations WHERE id = $1").bind(id).execute(&state.db).await;

	match result {
		Ok(res) if res.rows_affected() > 0 => {
			I18n::get().reload(&state.db).await;
			AuditLog::log(&state.db, LogEvent::TranslationDeleted, None, Some(EntityType::Translation), Some(id));
			ResponseBuilder::new(ResponseBody::Json(serde_json::json!({ "success": true }))).build()
		}
		Ok(_) => ErrorBuilder::new(ErrorCode::TranslationNotFound).build(),
		Err(e) => {
			eprintln!("[I18N] Failed to delete translation: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
