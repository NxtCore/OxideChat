use argon2::{
	Algorithm, Argon2, Params, Version,
	password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use sqlx::postgres::PgPool;
use tower_cookies::{Cookie, Cookies, cookie::SameSite};
use uuid::Uuid;

use crate::{
	routes::public::auth::{MAX_PASSWORD_LENGTH, MAX_USERNAME_LENGTH, MIN_PASSWORD_LENGTH, MIN_USERNAME_LENGTH, SESSION_COOKIE_NAME, SESSION_DURATION_DAYS},
	utils::response::ErrorCode,
};

const ARGON2_MEMORY_KIB: u32 = 65536;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;

/// Argon2id with explicitly pinned parameters.
fn argon2() -> Argon2<'static> {
	let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, ARGON2_PARALLELISM, None).unwrap_or_default();
	Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a password using Argon2id.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
	let salt = SaltString::generate(&mut OsRng);
	let hash = argon2().hash_password(password.as_bytes(), &salt)?;
	Ok(hash.to_string())
}

/// Verify a password against a hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
	let parsed_hash = PasswordHash::new(hash)?;
	Ok(argon2().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

/// Determine whether cookies should be marked `Secure` based on `BASE_URL`.
///
/// Returns `true` when `BASE_URL` uses the `https` scheme, `false` when it uses
/// `http`. If `BASE_URL` is unset or cannot be parsed, returns `true`, since
/// that indicates production intent where `Secure` cookies are expected.
#[must_use]
pub fn cookie_secure_from_env() -> bool {
	match std::env::var("BASE_URL") {
		Ok(base_url) => cookie_secure_for_base_url(&base_url),
		Err(_) => true,
	}
}

/// Derive the cookie `Secure` flag from a base URL's scheme.
///
/// `http` yields `false`, any other scheme (or no scheme) yields `true`.
#[must_use]
pub fn cookie_secure_for_base_url(base_url: &str) -> bool {
	match base_url.split_once("://") {
		Some((scheme, _)) => !scheme.eq_ignore_ascii_case("http"),
		None => true,
	}
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
	cookie.set_same_site(SameSite::Lax);
	cookie.set_secure(cookie_secure_from_env());
	cookie.set_max_age(tower_cookies::cookie::time::Duration::days(SESSION_DURATION_DAYS));

	cookies.add(cookie);
	Ok(())
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
