use argon2::{
	Argon2,
	password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{Json, http::StatusCode};
use chrono::{Duration, Utc};
use sqlx::postgres::PgPool;
use tower_cookies::{Cookie, Cookies, cookie::SameSite};
use uuid::Uuid;

use crate::{
	i18n::I18n,
	routes::public::auth::{MAX_PASSWORD_LENGTH, MAX_USERNAME_LENGTH, MIN_PASSWORD_LENGTH, MIN_USERNAME_LENGTH, SESSION_COOKIE_NAME, SESSION_DURATION_DAYS},
	types::{CountRow, RoleNameRow, User, UserResponse},
	utils::response::ErrorCode,
};

/// Hash a password using Argon2id.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
	let salt = SaltString::generate(&mut OsRng);
	let argon2 = Argon2::default();
	let hash = argon2.hash_password(password.as_bytes(), &salt)?;
	Ok(hash.to_string())
}

/// Verify a password against a hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
	let parsed_hash = PasswordHash::new(hash)?;
	Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

/// Check if any users exist in the database.
pub async fn users_exist(pool: &PgPool) -> Result<bool, sqlx::Error> {
	let row: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM users").fetch_one(pool).await?;
	Ok(row.count > 0)
}

/// Get user roles by user ID.
pub async fn get_user_roles(pool: &PgPool, user_id: &Uuid) -> Result<Vec<String>, sqlx::Error> {
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
///
/// Note: Multiple concurrent sessions per user are allowed by design.
/// This enables users to be logged in on multiple devices simultaneously.
/// Expired sessions are automatically cleaned up by the background job scheduler.
/// A "logout all devices" feature can be added later if needed.
pub async fn create_session(pool: &PgPool, cookies: &Cookies, user_id: &Uuid) -> Result<(), sqlx::Error> {
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
	// SameSite::Lax provides basic CSRF protection for non-GET requests from other origins.
	// TODO: Consider using SameSite::Strict for stronger protection, or implementing
	// CSRF tokens for state-changing operations if cross-origin POST is needed.
	cookie.set_same_site(SameSite::Lax);

	// Set secure flag based on environment (defaults to true for security)
	let is_secure = std::env::var("COOKIE_SECURE").map(|v| v != "false").unwrap_or(true);
	cookie.set_secure(is_secure);

	cookies.add(cookie);
	Ok(())
}

/// Convert a User to a UserResponse with roles.
pub async fn user_to_response(pool: &PgPool, user: &User) -> Result<UserResponse, sqlx::Error> {
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

/// Validate email format using a simple regex pattern.
pub fn validate_email(email: &str) -> bool {
	// Basic email validation: contains @ and has content before and after
	let parts: Vec<&str> = email.split('@').collect();
	if parts.len() != 2 {
		return false;
	}
	let (local, domain) = (parts[0], parts[1]);

	// Check basic requirements
	!local.is_empty() && !domain.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && email.len() <= 254
}

/// Validate username format.
pub fn validate_username(username: &str) -> Result<(), ErrorCode> {
	if username.len() < MIN_USERNAME_LENGTH {
		return Err(ErrorCode::UsernameTooShort);
	}
	if username.len() > MAX_USERNAME_LENGTH {
		return Err(ErrorCode::UsernameTooLong);
	}
	// Allow alphanumeric, underscores, and hyphens
	if !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
		return Err(ErrorCode::UsernameInvalid);
	}
	Ok(())
}

/// Validate password strength following NIST guidelines.
///
/// Requirements:
/// - Minimum 8 characters
/// - Maximum 128 characters
/// - At least one uppercase letter
/// - At least one lowercase letter
/// - At least one digit
/// - At least one special character
pub fn validate_password(password: &str) -> Result<(), ErrorCode> {
	if password.len() < MIN_PASSWORD_LENGTH {
		return Err(ErrorCode::PasswordTooShort);
	}
	if password.len() > MAX_PASSWORD_LENGTH {
		return Err(ErrorCode::PasswordTooLong);
	}
	if !password.chars().any(|c| c.is_uppercase()) {
		return Err(ErrorCode::PasswordNoUppercase);
	}
	if !password.chars().any(|c| c.is_lowercase()) {
		return Err(ErrorCode::PasswordNoLowercase);
	}
	if !password.chars().any(|c| c.is_ascii_digit()) {
		return Err(ErrorCode::PasswordNoDigit);
	}
	if !password.chars().any(|c| !c.is_alphanumeric()) {
		return Err(ErrorCode::PasswordNoSpecial);
	}
	Ok(())
}
