//! Authentication route handlers.
//!
//! Handles user setup, registration, login, and logout.

use crate::AppState;
use crate::types::{AuthResponse, CountRow, LoginRequest, MessageResponse, RegisterRequest, RoleNameRow, SetupRequest, User, UserResponse};
use argon2::{
	Argon2,
	password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies, cookie::SameSite};
use uuid::Uuid;

const SESSION_COOKIE_NAME: &str = "oxide_session";
const SESSION_DURATION_DAYS: i64 = 7;

/// Hash a password using Argon2id.
fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
	let salt = SaltString::generate(&mut OsRng);
	let argon2 = Argon2::default();
	let hash = argon2.hash_password(password.as_bytes(), &salt)?;
	Ok(hash.to_string())
}

/// Verify a password against a hash.
fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
	let parsed_hash = PasswordHash::new(hash)?;
	Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

/// Check if any users exist in the database.
async fn users_exist(pool: &PgPool) -> Result<bool, sqlx::Error> {
	let row: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM users").fetch_one(pool).await?;
	Ok(row.count > 0)
}

/// Get user roles by user ID.
async fn get_user_roles(pool: &PgPool, user_id: &Uuid) -> Result<Vec<String>, sqlx::Error> {
	let roles: Vec<RoleNameRow> = sqlx::query_as(
		"SELECT r.name FROM roles r
         INNER JOIN user_roles ur ON r.id = ur.role_id
         WHERE ur.user_id = $1",
	)
	.bind(user_id)
	.fetch_all(pool)
	.await?;
	Ok(roles.into_iter().map(|r| r.name).collect())
}

/// Create a session for a user and set the session cookie.
async fn create_session(pool: &PgPool, cookies: &Cookies, user_id: &Uuid) -> Result<(), sqlx::Error> {
	let session_id = Uuid::new_v4();
	let expires_at = Utc::now() + Duration::days(SESSION_DURATION_DAYS);

	sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
		.bind(session_id)
		.bind(user_id)
		.bind(expires_at)
		.execute(pool)
		.await?;

	let mut cookie = Cookie::new(SESSION_COOKIE_NAME, session_id.to_string());
	cookie.set_path("/");
	cookie.set_http_only(true);
	cookie.set_same_site(SameSite::Lax);
	// In production, set secure to true
	// cookie.set_secure(true);

	cookies.add(cookie);
	Ok(())
}

/// Convert a User to a UserResponse with roles.
async fn user_to_response(pool: &PgPool, user: &User) -> Result<UserResponse, sqlx::Error> {
	let roles = get_user_roles(pool, &user.id).await?;
	Ok(UserResponse {
		id: user.id,
		email: user.email.clone(),
		username: user.username.clone(),
		auth_method: user.auth_method.clone(),
		roles,
		created_at: user.created_at,
	})
}

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
					message: "Setup has already been completed".to_string(),
				}),
			)
				.into_response();
		}
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Database error: {e}"),
				}),
			)
				.into_response();
		}
		_ => {}
	}

	// Hash password
	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Password hashing error: {e}"),
				}),
			)
				.into_response();
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
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Failed to create user: {e}"),
				}),
			)
				.into_response();
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
		return (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to assign admin role: {e}"),
			}),
		)
			.into_response();
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		return (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to create session: {e}"),
			}),
		)
			.into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::CREATED, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to fetch user roles: {e}"),
			}),
		)
			.into_response(),
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
					message: "Setup must be completed first".to_string(),
				}),
			)
				.into_response();
		}
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Database error: {e}"),
				}),
			)
				.into_response();
		}
		_ => {}
	}

	// Hash password
	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Password hashing error: {e}"),
				}),
			)
				.into_response();
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
				"Email or username already taken".to_string()
			} else {
				format!("Failed to create user: {e}")
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
		return (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to assign user role: {e}"),
			}),
		)
			.into_response();
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		return (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to create session: {e}"),
			}),
		)
			.into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::CREATED, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to fetch user roles: {e}"),
			}),
		)
			.into_response(),
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
					message: "Invalid email or password".to_string(),
				}),
			)
				.into_response();
		}
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Database error: {e}"),
				}),
			)
				.into_response();
		}
	};

	// Check if user has a password (local auth)
	let password_hash = match &user.password_hash {
		Some(hash) => hash,
		None => {
			return (
				StatusCode::UNAUTHORIZED,
				Json(MessageResponse {
					message: "This account uses external authentication".to_string(),
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
					message: "Invalid email or password".to_string(),
				}),
			)
				.into_response();
		}
		Err(e) => {
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(MessageResponse {
					message: format!("Password verification error: {e}"),
				}),
			)
				.into_response();
		}
	}

	// Create session
	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		return (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to create session: {e}"),
			}),
		)
			.into_response();
	}

	// Return user response
	match user_to_response(&state.db, &user).await {
		Ok(user_response) => (StatusCode::OK, Json(AuthResponse { user: user_response })).into_response(),
		Err(e) => (
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(MessageResponse {
				message: format!("Failed to fetch user roles: {e}"),
			}),
		)
			.into_response(),
	}
}

/// POST /api/v1/auth/logout
///
/// Invalidate the current session.
pub async fn logout(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
		if let Ok(session_id) = Uuid::parse_str(session_cookie.value()) {
			// Delete session from database
			let _ = sqlx::query("DELETE FROM sessions WHERE id = $1").bind(session_id).execute(&state.db).await;
		}
	}

	// Remove cookie
	cookies.remove(Cookie::from(SESSION_COOKIE_NAME));

	(
		StatusCode::OK,
		Json(MessageResponse {
			message: "Logged out successfully".to_string(),
		}),
	)
}

/// Get the current user from the session cookie.
///
/// Returns None if no valid session exists.
pub async fn get_current_user(pool: &PgPool, cookies: &Cookies) -> Option<User> {
	let session_cookie = cookies.get(SESSION_COOKIE_NAME)?;
	let session_id = Uuid::parse_str(session_cookie.value()).ok()?;

	// Find valid session
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
