//! User route handlers.
//!
//! Handles user-related endpoints.

use crate::routes::public::auth::get_current_user;
use crate::types::MessageResponse;
use crate::{AppState, utils::auth::user_to_response};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

/// GET /api/v1/users/@me
///
/// Get the current authenticated user.
///
/// # Errors
///
/// Returns 401 if not authenticated.
pub async fn get_me(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => {
			return (
				StatusCode::UNAUTHORIZED,
				Json(MessageResponse {
					message: crate::i18n::I18n::get().translate("auth.errors.not_authenticated", &None),
				}),
			)
				.into_response();
		}
	};

	// Use shared helper function to build user response with roles
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::OK, Json(user_response)).into_response(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch roles for user {}: {e}", user.id);
			(
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: crate::i18n::I18n::get().translate("auth.errors.internal_error", &None),
				}),
			)
				.into_response()
		}
	}
}
