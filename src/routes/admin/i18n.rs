//! Admin i18n endpoints.
//!
//! Provides endpoints for managing translations.

use crate::AppState;
use crate::i18n::I18n;
use crate::types::{IdRow, Translation, TranslationsResponse, UpsertTranslationRequest};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
};
use std::sync::Arc;

/// GET /api/v1/admin/i18n
///
/// List all translations.
pub async fn list_translations(State(state): State<Arc<AppState>>) -> impl IntoResponse {
	let translations: Vec<Translation> = sqlx::query_as("SELECT id, language, key_path, value, is_override FROM i18n_translations ORDER BY language, key_path")
		.fetch_all(&state.db)
		.await
		.unwrap_or_default();

	Json(TranslationsResponse { translations })
}

/// PUT /api/v1/admin/i18n/translations
///
/// Create or update a translation. Reloads translations after change.
pub async fn upsert_translation(State(state): State<Arc<AppState>>, Json(payload): Json<UpsertTranslationRequest>) -> impl IntoResponse {
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
			// Reload translations so changes are immediately available
			I18n::get().reload(&state.db).await;
			(StatusCode::OK, Json(serde_json::json!({ "id": row.id, "success": true })))
		}
		Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
	}
}

/// DELETE /api/v1/admin/i18n/translations/:id
///
/// Delete a translation by ID. Reloads translations after change.
pub async fn delete_translation(State(state): State<Arc<AppState>>, Path(id): Path<sqlx::types::Uuid>) -> impl IntoResponse {
	let result = sqlx::query("DELETE FROM i18n_translations WHERE id = $1").bind(id).execute(&state.db).await;

	match result {
		Ok(res) if res.rows_affected() > 0 => {
			// Reload translations so changes are immediately available
			I18n::get().reload(&state.db).await;
			(StatusCode::OK, Json(serde_json::json!({ "success": true })))
		}
		Ok(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Translation not found" }))),
		Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
	}
}
