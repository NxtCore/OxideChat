//! OAuth authentication route handlers.
//!
//! Handles OAuth authorization flow for Google and Discord providers.

use crate::AppState;
use crate::config::OAuthProvider;
use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::User;
use crate::types::oauth::{OAuthCallbackParams, OAuthState, OAuthUserInfo};
use crate::utils::auth::create_session;
use crate::utils::oauth::{self, OAuthError};
use crate::utils::response::{ErrorBuilder, ErrorCode};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};

const OAUTH_STATE_COOKIE: &str = "oxidechat_oauth_state";

/// GET /api/v1/auth/oauth/{provider}
///
/// Initiates OAuth flow by redirecting to provider's authorization URL.
/// Sets a secure cookie with CSRF state and PKCE verifier.
///
/// # Errors
///
/// Returns 400 if provider is invalid or not configured.
pub async fn oauth_init(Path(provider): Path<String>, cookies: Cookies) -> Response {
	// Parse provider
	let oauth_provider = match OAuthProvider::from_str(&provider) {
		Some(p) => p,
		None => {
			return ErrorBuilder::new(ErrorCode::InvalidProvider).build();
		}
	};

	// Generate auth URL with PKCE
	let (auth_url, state) = match oauth::generate_auth_url(oauth_provider) {
		Ok(result) => result,
		Err(OAuthError::ProviderNotConfigured) => {
			return ErrorBuilder::new(ErrorCode::ProviderNotConfigured).build();
		}
		Err(e) => {
			eprintln!("[OAUTH] Failed to generate auth URL: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Store state in secure cookie
	let state_json = match serde_json::to_string(&state) {
		Ok(json) => json,
		Err(e) => {
			eprintln!("[OAUTH] Failed to serialize state: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut cookie = Cookie::new(OAUTH_STATE_COOKIE, state_json);
	cookie.set_http_only(true);
	cookie.set_secure(true);
	cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
	cookie.set_path("/");
	cookie.set_max_age(tower_cookies::cookie::time::Duration::minutes(10));
	cookies.add(cookie);

	// Redirect to provider
	Redirect::temporary(&auth_url).into_response()
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
pub async fn oauth_callback(Path(provider): Path<String>, Query(params): Query<OAuthCallbackParams>, State(state): State<Arc<AppState>>, cookies: Cookies) -> Response {
	// Parse provider
	let oauth_provider = match OAuthProvider::from_str(&provider) {
		Some(p) => p,
		None => {
			return ErrorBuilder::new(ErrorCode::InvalidProvider).build();
		}
	};

	// Get state from cookie
	let state_cookie = match cookies.get(OAUTH_STATE_COOKIE) {
		Some(c) => c,
		None => {
			eprintln!("[OAUTH] Missing state cookie");
			return ErrorBuilder::new(ErrorCode::OAuthStateMismatch).build();
		}
	};

	let oauth_state: OAuthState = match serde_json::from_str(state_cookie.value()) {
		Ok(s) => s,
		Err(e) => {
			eprintln!("[OAUTH] Failed to parse state cookie: {e}");
			return ErrorBuilder::new(ErrorCode::OAuthStateMismatch).build();
		}
	};

	// Validate CSRF token
	if params.state != oauth_state.csrf_token {
		eprintln!("[OAUTH] CSRF state mismatch");
		AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
		return ErrorBuilder::new(ErrorCode::OAuthStateMismatch).build();
	}

	// Clear the state cookie
	cookies.remove(Cookie::from(OAUTH_STATE_COOKIE));

	// Exchange code for access token
	let access_token = match oauth::exchange_code(oauth_provider, &params.code, &oauth_state).await {
		Ok(token) => token,
		Err(e) => {
			eprintln!("[OAUTH] Token exchange failed: {e}");
			AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
			return ErrorBuilder::new(ErrorCode::OAuthTokenError).build();
		}
	};

	// Fetch user info from provider
	let user_info = match oauth::fetch_user_info(oauth_provider, &access_token).await {
		Ok(info) => info,
		Err(e) => {
			eprintln!("[OAUTH] Failed to fetch user info: {e}");
			AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
			return ErrorBuilder::new(ErrorCode::OAuthUserInfoError).build();
		}
	};

	// Validate that email is provided by OAuth provider
	if user_info.email.is_none() {
		eprintln!("[OAUTH] OAuth provider did not return an email address");
		AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
		return ErrorBuilder::new(ErrorCode::OAuthUserInfoError).build();
	}

	// Find or create user
	match find_or_create_user(&state.db, &user_info, &cookies).await {
		Ok(user) => {
			// Log successful OAuth login
			AuditLog::log(&state.db, LogEvent::OAuthLoginSuccess, Some(user.id), Some(EntityType::User), Some(user.id));

			// Return success with redirect
			let redirect_url = oauth_state.redirect_after.as_deref().unwrap_or("/");

			// Create session
			if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
				eprintln!("[OAUTH] Failed to create session: {e}");
				return ErrorBuilder::new(ErrorCode::DatabaseError).build();
			}

			// For API, redirect to frontend
			Redirect::temporary(redirect_url).into_response()
		}
		Err(e) => {
			eprintln!("[OAUTH] Failed to find/create user: {e}");
			AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, None, None, None);
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Find existing user by OAuth identity or email, or create a new user.
/// Links the OAuth identity to the user if not already linked.
async fn find_or_create_user(pool: &sqlx::PgPool, user_info: &OAuthUserInfo, _cookies: &Cookies) -> Result<User, sqlx::Error> {
	// Check if we have an existing identity for this provider+user_id
	let existing_identity: Option<uuid::Uuid> = sqlx::query_scalar("SELECT user_id FROM user_identities WHERE provider = $1 AND provider_user_id = $2")
		.bind(&user_info.provider)
		.bind(&user_info.provider_user_id)
		.fetch_optional(pool)
		.await?;

	if let Some(user_id) = existing_identity {
		// User exists with this OAuth identity, fetch and return
		let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(user_id).fetch_one(pool).await?;
		return Ok(user);
	}

	// No existing identity - check if email matches an existing user
	let email = user_info.email.as_ref();

	if let Some(email) = email {
		let existing_user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1").bind(email).fetch_optional(pool).await?;

		if let Some(user) = existing_user {
			// Link OAuth identity to existing user (auto-link strategy)
			sqlx::query(
				"INSERT INTO user_identities (user_id, provider, provider_user_id, provider_data) 
				 VALUES ($1, $2, $3, $4)",
			)
			.bind(user.id)
			.bind(&user_info.provider)
			.bind(&user_info.provider_user_id)
			.bind(&user_info.raw_data)
			.execute(pool)
			.await?;

			AuditLog::log(pool, LogEvent::OAuthIdentityLinked, Some(user.id), Some(EntityType::User), Some(user.id));

			return Ok(user);
		}
	}

	// No existing user - create new one
	// Email is guaranteed to exist because we validate it in oauth_callback
	let email = email.expect("Email should be present after validation");

	let username = user_info
		.username
		.as_deref()
		.map(String::from)
		.unwrap_or_else(|| format!("user_{}", &user_info.provider_user_id[..8.min(user_info.provider_user_id.len())]));

	// Make username unique if it already exists
	let unique_username = make_unique_username(pool, &username).await?;

	// Create user
	let user: User = sqlx::query_as(
		"INSERT INTO users (email, username, auth_method) 
		 VALUES ($1, $2, 'oauth') 
		 RETURNING *",
	)
	.bind(&email)
	.bind(&unique_username)
	.fetch_one(pool)
	.await?;

	// Assign default user role
	sqlx::query(
		"INSERT INTO user_roles (user_id, role_id) 
		 SELECT $1, id FROM roles WHERE name = 'user'",
	)
	.bind(user.id)
	.execute(pool)
	.await?;

	// Create OAuth identity link
	sqlx::query(
		"INSERT INTO user_identities (user_id, provider, provider_user_id, provider_data) 
		 VALUES ($1, $2, $3, $4)",
	)
	.bind(user.id)
	.bind(&user_info.provider)
	.bind(&user_info.provider_user_id)
	.bind(&user_info.raw_data)
	.execute(pool)
	.await?;

	AuditLog::log(pool, LogEvent::OAuthAccountCreated, Some(user.id), Some(EntityType::User), Some(user.id));

	Ok(user)
}

/// Generate a unique username by appending numbers if necessary.
async fn make_unique_username(pool: &sqlx::PgPool, base_username: &str) -> Result<String, sqlx::Error> {
	let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
		.bind(base_username)
		.fetch_one(pool)
		.await?;

	if !exists {
		return Ok(base_username.to_string());
	}

	// Username taken, try with numbers
	for i in 1..1000 {
		let candidate = format!("{base_username}{i}");
		let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
			.bind(&candidate)
			.fetch_one(pool)
			.await?;

		if !exists {
			return Ok(candidate);
		}
	}

	// Fallback to UUID suffix
	Ok(format!("{base_username}_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x")))
}
