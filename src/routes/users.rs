//! User route handlers.
//!
//! Handles user-related endpoints.

use crate::AppState;
use crate::routes::auth::get_current_user;
use crate::types::{MessageResponse, UserResponse};
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
					message: "Not authenticated".to_string(),
				}),
			)
				.into_response();
		}
	};

	// Get user roles
	let roles: Vec<String> = match sqlx::query_scalar(
		"SELECT r.name FROM roles r
         INNER JOIN user_roles ur ON r.id = ur.role_id
         WHERE ur.user_id = $1",
	)
	.bind(user.id)
	.fetch_all(&state.db)
	.await
	{
		Ok(roles) => roles,
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Failed to fetch roles: {e}"),
				}),
			)
				.into_response();
		}
	};

	(
		StatusCode::OK,
		Json(UserResponse {
			id: user.id,
			email: user.email,
			username: user.username,
			auth_method: user.auth_method,
			roles,
			created_at: user.created_at,
		}),
	)
		.into_response()
}
