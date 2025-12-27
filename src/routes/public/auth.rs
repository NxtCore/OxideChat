//! Authentication route handlers.
//!
//! Handles user setup, registration, login, and logout.

use crate::AppState;
use crate::i18n::I18n;
use crate::types::{AuthResponse, LoginRequest, MessageResponse, RegisterRequest, SetupRequest, User};
use crate::utils::auth::{
	create_session, hash_password, internal_error, user_to_response, users_exist, validate_email, validate_password, validate_username, verify_password,
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use sqlx::PgPool;
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

pub const SESSION_COOKIE_NAME: &str = "oxidechat_session";
pub const SESSION_DURATION_DAYS: i64 = 7;
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 128;
pub const MAX_USERNAME_LENGTH: usize = 32;
pub const MIN_USERNAME_LENGTH: usize = 3;

/// POST /api/v1/auth/setup
///
/// Create the first admin user. Fails if any users already exist.
///
/// # Errors
///
/// Returns 400 if setup has already been completed.
/// Returns 500 on database or hashing errors.
pub async fn setup(State(state): State<Arc<AppState>>, cookies: Cookies, Json(payload): Json<SetupRequest>) -> impl IntoResponse {
	// Check if setup already completed
	match users_exist(&state.db).await {
		Ok(true) => {
			return (
				StatusCode::BAD_REQUEST,
				Json(MessageResponse {
					message: I18n::get().translate("auth.errors.setup_completed", &None),
				}),
			)
				.into_response();
		}
		Err(e) => {
			eprintln!("[AUTH] Database error checking users: {e}");
			return internal_error().into_response();
		}
		_ => {}
	}

	// Validate email
	if !validate_email(&payload.email) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate("auth.errors.invalid_email", &None),
			}),
		)
			.into_response();
	}

	// Validate username
	if let Err(key) = validate_username(&payload.username) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate(key, &None),
			}),
		)
			.into_response();
	}

	// Validate password
	if let Err(key) = validate_password(&payload.password) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate(key, &None),
			}),
		)
			.into_response();
	}

	// Hash password
	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			eprintln!("[AUTH] Password hashing error: {e}");
			return internal_error().into_response();
		}
	};

	// Create user
	let user: User = match sqlx::query_as(
		"INSERT INTO users (email, username, password_hash, auth_method)
         VALUES ($1, $2, $3, 'local')
         RETURNING *",
	)
	.bind(&payload.email)
	.bind(&payload.username)
	.bind(&password_hash)
	.fetch_one(&state.db)
	.await
	{
		Ok(user) => user,
		Err(e) => {
			eprintln!("[AUTH] Failed to create user: {e}");
			return internal_error().into_response();
		}
	};

	// Assign admin role
	if let Err(e) = sqlx::query(
		"INSERT INTO user_roles (user_id, role_id)
         SELECT $1, id FROM roles WHERE name = 'admin'",
	)
	.bind(user.id)
	.execute(&state.db)
	.await
	{
		eprintln!("[AUTH] Failed to assign admin role: {e}");
		return internal_error().into_response();
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return internal_error().into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::CREATED, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			internal_error().into_response()
		}
	}
}

/// POST /api/v1/auth/register
///
/// Register a new user. Requires setup to be completed first.
///
/// # Errors
///
/// Returns 400 if setup not completed or email/username taken.
/// Returns 500 on database or hashing errors.
pub async fn register(State(state): State<Arc<AppState>>, cookies: Cookies, Json(payload): Json<RegisterRequest>) -> impl IntoResponse {
	// Check if setup completed
	match users_exist(&state.db).await {
		Ok(false) => {
			return (
				StatusCode::BAD_REQUEST,
				Json(MessageResponse {
					message: I18n::get().translate("auth.errors.setup_required", &None),
				}),
			)
				.into_response();
		}
		Err(e) => {
			eprintln!("[AUTH] Database error checking users: {e}");
			return internal_error().into_response();
		}
		_ => {}
	}

	// Validate email
	if !validate_email(&payload.email) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate("auth.errors.invalid_email", &None),
			}),
		)
			.into_response();
	}

	// Validate username
	if let Err(key) = validate_username(&payload.username) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate(key, &None),
			}),
		)
			.into_response();
	}

	// Validate password
	if let Err(key) = validate_password(&payload.password) {
		return (
			StatusCode::BAD_REQUEST,
			Json(MessageResponse {
				message: I18n::get().translate(key, &None),
			}),
		)
			.into_response();
	}

	// Hash password
	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			eprintln!("[AUTH] Password hashing error: {e}");
			return internal_error().into_response();
		}
	};

	// Create user
	let user: User = match sqlx::query_as(
		"INSERT INTO users (email, username, password_hash, auth_method)
         VALUES ($1, $2, $3, 'local')
         RETURNING *",
	)
	.bind(&payload.email)
	.bind(&payload.username)
	.bind(&password_hash)
	.fetch_one(&state.db)
	.await
	{
		Ok(user) => user,
		Err(e) => {
			let message = if e.to_string().contains("duplicate key") {
				I18n::get().translate("auth.errors.email_or_username_taken", &None)
			} else {
				eprintln!("[AUTH] Failed to create user: {e}");
				I18n::get().translate("auth.errors.internal_error", &None)
			};
			return (StatusCode::BAD_REQUEST, Json(MessageResponse { message })).into_response();
		}
	};

	// Assign user role
	if let Err(e) = sqlx::query(
		"INSERT INTO user_roles (user_id, role_id)
         SELECT $1, id FROM roles WHERE name = 'user'",
	)
	.bind(user.id)
	.execute(&state.db)
	.await
	{
		eprintln!("[AUTH] Failed to assign user role: {e}");
		return internal_error().into_response();
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return internal_error().into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::CREATED, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			internal_error().into_response()
		}
	}
}

/// POST /api/v1/auth/login
///
/// Authenticate a user with email and password.
///
/// # Errors
///
/// Returns 401 if credentials are invalid.
/// Returns 500 on database errors.
pub async fn login(State(state): State<Arc<AppState>>, cookies: Cookies, Json(payload): Json<LoginRequest>) -> impl IntoResponse {
	// Find user by email
	let user: User = match sqlx::query_as("SELECT * FROM users WHERE email = $1")
		.bind(&payload.email)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(user)) => user,
		Ok(None) => {
			return (
				StatusCode::UNAUTHORIZED,
				Json(MessageResponse {
					message: I18n::get().translate("auth.errors.invalid_credentials", &None),
				}),
			)
				.into_response();
		}
		Err(e) => {
			eprintln!("[AUTH] Database error during login: {e}");
			return internal_error().into_response();
		}
	};

	// Check if user has a password (local auth)
	let password_hash = match &user.password_hash {
		Some(hash) => hash,
		None => {
			return (
				StatusCode::UNAUTHORIZED,
				Json(MessageResponse {
					message: I18n::get().translate("auth.errors.external_auth", &None),
				}),
			)
				.into_response();
		}
	};

	// Verify password
	match verify_password(&payload.password, password_hash) {
		Ok(true) => {}
		Ok(false) => {
			return (
				StatusCode::UNAUTHORIZED,
				Json(MessageResponse {
					message: I18n::get().translate("auth.errors.invalid_credentials", &None),
				}),
			)
				.into_response();
		}
		Err(e) => {
			eprintln!("[AUTH] Password verification error: {e}");
			return internal_error().into_response();
		}
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return internal_error().into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::OK, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			internal_error().into_response()
		}
	}
}

/// POST /api/v1/auth/logout
///
/// Invalidate the current session.
pub async fn logout(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
		if let Ok(session_id) = Uuid::parse_str(session_cookie.value()) {
			let _ = sqlx::query("DELETE FROM sessions WHERE id = $1").bind(session_id).execute(&state.db).await;
		}
	}

	cookies.remove(Cookie::from(SESSION_COOKIE_NAME));

	(
		StatusCode::OK,
		Json(MessageResponse {
			message: I18n::get().translate("auth.messages.logout_success", &None),
		}),
	)
}

/// Get the current user from the session cookie.
///
/// Returns None if no valid session exists.
pub async fn get_current_user(pool: &PgPool, cookies: &Cookies) -> Option<User> {
	let session_cookie = cookies.get(SESSION_COOKIE_NAME)?;
	let session_id = Uuid::parse_str(session_cookie.value()).ok()?;

	let user: User = sqlx::query_as(
		"SELECT u.* FROM users u
         INNER JOIN sessions s ON u.id = s.user_id
         WHERE s.id = $1 AND s.expires_at > NOW()",
	)
	.bind(session_id)
	.fetch_optional(pool)
	.await
	.ok()??;

	Some(user)
}
