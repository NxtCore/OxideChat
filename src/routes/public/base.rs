//! Base endpoint handler.
//!
//! Returns application base data including translations.

use crate::i18n::I18n;
use crate::types::JobState;
use crate::types::{BaseResponse, CountRow};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::State;
use axum::response::IntoResponse;
use std::sync::Arc;

/// GET /api/v1/base
///
/// Returns base application data including translations and setup status.
///
/// # Errors
///
/// Returns 500 if the database query fails.
pub async fn get_base(State(state): State<Arc<JobState>>) -> impl IntoResponse {
	let i18n = I18n::get().all();

	// Check if any users exist (needs_setup = true if no users)
	let needs_setup = match sqlx::query_as::<_, CountRow>("SELECT COUNT(*) as count FROM users").fetch_one(&state.db).await {
		Ok(row) => row.count == 0,
		Err(e) => {
			// Database error - don't assume setup is needed, return error
			eprintln!("[BASE] Database error checking users: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	let roles = crate::types::Role::list_all(&state.db).await.unwrap_or_else(|e| {
		eprintln!("[BASE] Failed to list roles: {e}");
		vec![]
	});

	ResponseBuilder::new(ResponseBody::Json(BaseResponse {
		i18n,
		language: crate::config::Config::get().language().to_string(),
		needs_setup,
		oauth_providers: crate::config::Config::get().get_configured_oauth_providers(),
		roles,
	}))
	.build()
}
