use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::utils::auth::user_to_response;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{extract::State, response::IntoResponse};
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
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	match user_to_response(&state.db, &user).await {
		Ok(user_response) => ResponseBuilder::new(ResponseBody::Json(user_response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch roles for user {}: {e}", user.id);
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
