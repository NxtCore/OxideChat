//! OAuth authentication route handlers.
//!
//! Handles the sign-in flow for the providers admins have configured.

use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::oauth::{OAuthCallbackParams, OAuthProvider, OAuthState, SignInError, SignInKind};
use crate::types::{AppError, JobState, User};
use crate::utils::auth::{cookie_secure_from_env, create_session};
use crate::utils::oauth;
use crate::utils::response::ErrorCode;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};

const OAUTH_STATE_COOKIE: &str = "oxidechat_oauth_state";

async fn enabled_provider(state: &JobState, slug: &str) -> Result<OAuthProvider, AppError> {
	Ok(OAuthProvider::find_enabled_by_slug(&state.db, slug).await?.ok_or(ErrorCode::ProviderNotConfigured)?)
}

/// GET /api/v1/auth/oauth/{slug}
///
/// Initiates OAuth flow by redirecting to provider's authorization URL.
/// Sets a secure cookie with CSRF state and PKCE verifier.
///
/// # Errors
///
/// Returns 400 if the provider does not exist or is disabled.
pub async fn oauth_init(Path(slug): Path<String>, State(state): State<Arc<JobState>>, cookies: Cookies) -> Result<Response, AppError> {
	let provider = enabled_provider(&state, &slug).await?;

	let (auth_url, oauth_state) = oauth::generate_auth_url(&provider).map_err(|e| {
		tracing::error!("[OAUTH] Failed to generate auth URL: {e}");
		ErrorCode::InternalError
	})?;

	let state_json = serde_json::to_string(&oauth_state).map_err(|e| {
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

fn read_state(cookies: &Cookies, params: &OAuthCallbackParams, slug: &str) -> Result<OAuthState, ErrorCode> {
	let cookie = cookies.get(OAUTH_STATE_COOKIE).ok_or(ErrorCode::OAuthStateMismatch)?;
	let oauth_state: OAuthState = serde_json::from_str(cookie.value()).map_err(|_| ErrorCode::OAuthStateMismatch)?;
	if params.state != oauth_state.csrf_token || oauth_state.provider_slug != slug {
		return Err(ErrorCode::OAuthStateMismatch);
	}
	Ok(oauth_state)
}

fn error_redirect(code: &str) -> Response {
	Redirect::temporary(&format!("/auth/error?code={code}")).into_response()
}

fn log_failure(state: &JobState, user: Option<&User>) {
	AuditLog::log(&state.db, LogEvent::OAuthLoginFailed, user.map(|u| u.id), user.map(|_| EntityType::User), user.map(|u| u.id));
}

/// GET /api/v1/auth/oauth/{slug}/callback
///
/// Handles OAuth callback:
/// 1. Validates CSRF state from cookie
/// 2. Exchanges code for access token
/// 3. Reads the user's profile from the provider
/// 4. Signs in, links by verified email or creates the account
/// 5. Creates session and redirects to app
pub async fn oauth_callback(
	Path(slug): Path<String>,
	Query(params): Query<OAuthCallbackParams>,
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
) -> Result<Response, AppError> {
	let provider = enabled_provider(&state, &slug).await?;
	let oauth_state = read_state(&cookies, &params, &slug).inspect_err(|_| {
		tracing::warn!("[OAUTH] Invalid state for provider {}", provider.slug);
		log_failure(&state, None);
	})?;
	cookies.remove(Cookie::from(OAUTH_STATE_COOKIE));

	let Some(code) = params.code.as_deref().filter(|_| params.error.is_none()) else {
		log_failure(&state, None);
		return Ok(error_redirect("oauth_token_error"));
	};

	let access_token = oauth::exchange_code(&provider, code, &oauth_state).await.map_err(|e| {
		tracing::warn!("[OAUTH] Token exchange failed: {e}");
		log_failure(&state, None);
		ErrorCode::OAuthTokenError
	})?;
	let user_info = oauth::fetch_user_info(&provider, &access_token).await.map_err(|e| {
		tracing::warn!("[OAUTH] Failed to fetch user info: {e}");
		log_failure(&state, None);
		ErrorCode::OAuthUserInfoError
	})?;

	match provider.sign_in(&state.db, &user_info).await {
		Ok((user, kind)) => {
			create_session(&state.db, &cookies, &user.id).await.map_err(|e| {
				tracing::error!("[OAUTH] Failed to create session: {e}");
				log_failure(&state, Some(&user));
				AppError::database(e)
			})?;
			log_success(&state, &user, kind);
			Ok(Redirect::temporary(oauth_state.redirect_after.as_deref().unwrap_or("/")).into_response())
		}
		Err(SignInError::Database(e)) => {
			tracing::error!("[OAUTH] Failed to sign in: {e}");
			log_failure(&state, None);
			Err(AppError::database(e))
		}
		Err(refused) => {
			log_failure(&state, None);
			Ok(error_redirect(match refused {
				SignInError::EmailNotVerified => "oauth_email_not_verified",
				SignInError::RegistrationClosed => "oauth_registration_closed",
				_ => "oauth_user_info_error",
			}))
		}
	}
}

fn log_success(state: &JobState, user: &User, kind: SignInKind) {
	let event = match kind {
		SignInKind::Existing => None,
		SignInKind::Linked => Some(LogEvent::OAuthAccountLinked),
		SignInKind::Created => Some(LogEvent::OAuthAccountCreated),
	};
	for event in event.into_iter().chain([LogEvent::OAuthLoginSuccess]) {
		AuditLog::log(&state.db, event, Some(user.id), Some(EntityType::User), Some(user.id));
	}
}
