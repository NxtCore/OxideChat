//! OAuth authentication route handlers.
//!
//! Handles OAuth authorization flow for Google and Discord providers.

use crate::config::OAuthProvider;
use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::{AppError, JobState, User};
use crate::types::oauth::{OAuthCallbackParams, OAuthState, OAuthUserInfo};
use crate::utils::auth::{cookie_secure_from_env, create_session};
use crate::utils::oauth::{self, OAuthError};
use crate::utils::response::ErrorCode;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};

const OAUTH_STATE_COOKIE: &str = "oxidechat_oauth_state";

#[derive(Debug)]
enum FindOrCreateError {
	EmailConflict,
	MissingEmail,
	Database(sqlx::Error),
}

impl From<sqlx::Error> for FindOrCreateError {
	fn from(err: sqlx::Error) -> Self {
		Self::Database(err)
	}
}

/// GET /api/v1/auth/oauth/{provider}
///
/// Initiates OAuth flow by redirecting to provider's authorization URL.
/// Sets a secure cookie with CSRF state and PKCE verifier.
///
/// # Errors
///
/// Returns 400 if provider is invalid or not configured.
pub async fn oauth_init(Path(provider): Path<String>, cookies: Cookies) -> Result<Response, AppError> {
	let oauth_provider = OAuthProvider::from_str(&provider).ok_or(ErrorCode::InvalidProvider)?;

	let (auth_url, state) = oauth::generate_auth_url(oauth_provider).map_err(|e| match e {
		OAuthError::ProviderNotConfigured => ErrorCode::ProviderNotConfigured,
		e => {
			tracing::error!("[OAUTH] Failed to generate auth URL: {e}");
			ErrorCode::InternalError
		}
	})?;

	let state_json = serde_json::to_string(&state).map_err(|e| {
		tracing::error!("[OAUTH] Failed to serialize state: {e}");
		ErrorCode::InternalError
	})?;

	let mut cookie = Cookie::new(OAUTH_STATE_COOKIE, state_json);
	cookie.set_http_only(true);
	cookie.set_secure(cookie_secure_from_env());
	cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
	cookie.set_path("/");
	cookie.set_max_age(tower_cookies::cookie::time::Duration::minutes(10));
	cookies.add(cookie);

	Ok(Redirect::temporary(&auth_url).into_response())
}

/// GET /api/v1/auth/oauth/{provider}/callback
///
/// Handles OAuth callback:
/// 1. Validates CSRF state from cookie
/// 2. Exchanges code for access token
/// 3. Fetches user info from provider
/// 4. Finds or creates user account
/// 5. Links OAuth identity if not already linked
/// 6. Creates session and redirects to app
#[allow(clippy::cognitive_complexity)]
pub async fn oauth_callback(
	Path(provider): Path<String>,
	Query(params): Query<OAuthCallbackParams>,
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
) -> Result<Response, AppError> {
	let oauth_provider = OAuthProvider::from_str(&provider).ok_or(ErrorCode::InvalidProvider)?;

	let state_cookie = cookies.get(OAUTH_STATE_COOKIE).ok_or_else(|| {
		tracing::warn!("[OAUTH] Missing state cookie");
		ErrorCode::OAuthStateMismatch
	})?;

	let oauth_state: OAuthState = serde_json::from_str(state_cookie.value()).map_err(|e| {
		tracing::warn!("[OAUTH] Failed to parse state cookie: {e}");
		ErrorCode::OAuthStateMismatch
	})?;

	if params.state != oauth_state.csrf_token {
		tracing::warn!("[OAUTH] CSRF state mismatch");
		AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
		return Err(ErrorCode::OAuthStateMismatch.into());
	}

	cookies.remove(Cookie::from(OAUTH_STATE_COOKIE));

	let oauth_failed = |code: ErrorCode| {
		AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
		code
	};

	let access_token = oauth::exchange_code(oauth_provider, &params.code, &oauth_state).await.map_err(|e| {
		tracing::warn!("[OAUTH] Token exchange failed: {e}");
		oauth_failed(ErrorCode::OAuthTokenError)
	})?;

	let user_info = oauth::fetch_user_info(oauth_provider, &access_token).await.map_err(|e| {
		tracing::warn!("[OAUTH] Failed to fetch user info: {e}");
		oauth_failed(ErrorCode::OAuthUserInfoError)
	})?;

	if user_info.email.is_none() || user_info.email_verified == Some(false) {
		tracing::warn!("[OAUTH] OAuth provider returned no usable email address");
		return Err(oauth_failed(ErrorCode::OAuthUserInfoError).into());
	}

	match find_or_create_user(&state.db, &user_info).await {
		Ok(user) => {
			let redirect_url = oauth_state.redirect_after.as_deref().unwrap_or("/");

			if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
				tracing::error!("[OAUTH] Failed to create session: {e}");
				AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, Some(user.id), Some(EntityType::User), Some(user.id));
				return Err(ErrorCode::DatabaseError.into());
			}

			AuditLog::log(&state.db, LogEvent::OAuthLoginSuccess, Some(user.id), Some(EntityType::User), Some(user.id));
			Ok(Redirect::temporary(redirect_url).into_response())
		}
		Err(FindOrCreateError::EmailConflict) => {
			tracing::warn!("[OAUTH] OAuth email matches existing account - requires manual linking");
			Ok(Redirect::temporary("/auth/error?code=oauth_email_conflict").into_response())
		}
		Err(FindOrCreateError::MissingEmail) => {
			tracing::warn!("[OAUTH] OAuth provider did not return an email address");
			Ok(Redirect::temporary("/auth/error?code=oauth_user_info_error").into_response())
		}
		Err(FindOrCreateError::Database(e)) => {
			tracing::error!("[OAUTH] Failed to find/create user: {e}");
			Err(oauth_failed(ErrorCode::DatabaseError).into())
		}
	}
}

/// Find existing user by OAuth identity, or create a new user.
///
/// Returns an error if the email matches an existing account that doesn't have
/// this OAuth identity linked - the user must manually link their accounts.
async fn find_or_create_user(pool: &sqlx::PgPool, user_info: &OAuthUserInfo) -> Result<User, FindOrCreateError> {
	if let Some(user) = User::find_by_identity(pool, &user_info.provider, &user_info.provider_user_id).await? {
		return Ok(user);
	}

	let email = user_info.email.as_deref().ok_or(FindOrCreateError::MissingEmail)?;

	if User::find_by_email(pool, email).await?.is_some() {
		return Err(FindOrCreateError::EmailConflict);
	}

	let username = user_info
		.username
		.as_deref()
		.map(String::from)
		.unwrap_or_else(|| format!("user_{}", &user_info.provider_user_id[..8.min(user_info.provider_user_id.len())]));

	let unique_username = make_unique_username(pool, &username).await?;

	let user = User::create_from_oauth(pool, email, &unique_username, user_info).await?;
	user.initialize_defaults(pool).await?;

	AuditLog::log(pool, LogEvent::OAuthAccountCreated, Some(user.id), Some(EntityType::User), Some(user.id));

	Ok(user)
}

/// Generate a unique username by appending numbers if necessary.
async fn make_unique_username(pool: &sqlx::PgPool, base_username: &str) -> Result<String, sqlx::Error> {
	if !User::username_exists(pool, base_username, None).await? {
		return Ok(base_username.to_string());
	}

	let mut i = 1u64;
	loop {
		let candidate = format!("{base_username}{i}");
		if !User::username_exists(pool, &candidate, None).await? {
			return Ok(candidate);
		}
		i += 1;
	}
}
