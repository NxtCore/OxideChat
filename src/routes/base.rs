//! Base endpoint handler.
//!
//! Returns application base data including translations.

use crate::AppState;
use crate::i18n::I18n;
use crate::types::{BaseResponse, CountRow};
use axum::{Json, extract::State};
use std::sync::Arc;

/// GET /api/v1/base
///
/// Returns base application data including translations and setup status.
pub async fn get_base(State(state): State<Arc<AppState>>) -> Json<BaseResponse> {
	let i18n = I18n::get().all();

	// Check if any users exist (needs_setup = true if no users)
	let needs_setup = match sqlx::query_as::<_, CountRow>("SELECT COUNT(*) as count FROM users").fetch_one(&state.db).await {
		Ok(row) => row.count == 0,
		Err(_) => true, // Assume setup needed if query fails
	};

	Json(BaseResponse { i18n, needs_setup })
}
