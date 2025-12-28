//! OAuth service for handling provider authentication flows.
//!
//! Provides utilities for building OAuth clients, generating authorization URLs,
//! exchanging codes for tokens, and fetching user information.

use crate::config::{Config, OAuthProvider};
use crate::types::oauth::{DiscordUserInfo, GoogleUserInfo, OAuthState, OAuthUserInfo};
use oauth2::{AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl};

/// OAuth-related errors.
#[derive(Debug)]
pub enum OAuthError {
	/// Provider is not configured
	ProviderNotConfigured,
	/// Invalid provider name
	InvalidProvider,
	/// Failed to build OAuth client
	ClientBuildError(String),
	/// Failed to exchange code for token
	TokenExchangeError(String),
	/// Failed to fetch user info
	UserInfoError(String),
	/// CSRF state mismatch
	StateMismatch,
	/// Email is required but not provided
	EmailRequired,
}

impl std::fmt::Display for OAuthError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::ProviderNotConfigured => write!(f, "OAuth provider not configured"),
			Self::InvalidProvider => write!(f, "Invalid OAuth provider"),
			Self::ClientBuildError(e) => write!(f, "Failed to build OAuth client: {e}"),
			Self::TokenExchangeError(e) => write!(f, "Failed to exchange code: {e}"),
			Self::UserInfoError(e) => write!(f, "Failed to fetch user info: {e}"),
			Self::StateMismatch => write!(f, "CSRF state mismatch"),
			Self::EmailRequired => write!(f, "Email is required for registration"),
		}
	}
}

impl std::error::Error for OAuthError {}

/// OAuth configuration for a provider.
struct ProviderConfig {
	client_id: String,
	client_secret: String,
	redirect_uri: String,
	auth_url: &'static str,
	token_url: &'static str,
	scopes: &'static [&'static str],
}

/// Get provider configuration from global config.
fn get_provider_config(provider: OAuthProvider) -> Result<ProviderConfig, OAuthError> {
	let config = Config::get();
	let values = config.values();

	if !config.is_oauth_provider_configured(provider) {
		return Err(OAuthError::ProviderNotConfigured);
	}

	fn get_value(db_value: &Option<String>, env_key: &str) -> String {
		db_value.as_ref().cloned().or_else(|| std::env::var(env_key).ok()).unwrap_or_default()
	}

	match provider {
		OAuthProvider::Google => Ok(ProviderConfig {
			client_id: get_value(&values.oauth_google_client_id, "OAUTH_GOOGLE_CLIENT_ID"),
			client_secret: get_value(&values.oauth_google_client_secret, "OAUTH_GOOGLE_CLIENT_SECRET"),
			redirect_uri: get_value(&values.oauth_google_redirect_uri, "OAUTH_GOOGLE_REDIRECT_URI"),
			auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
			token_url: "https://oauth2.googleapis.com/token",
			scopes: &["openid", "email", "profile"],
		}),
		OAuthProvider::Discord => Ok(ProviderConfig {
			client_id: get_value(&values.oauth_discord_client_id, "OAUTH_DISCORD_CLIENT_ID"),
			client_secret: get_value(&values.oauth_discord_client_secret, "OAUTH_DISCORD_CLIENT_SECRET"),
			redirect_uri: get_value(&values.oauth_discord_redirect_uri, "OAUTH_DISCORD_REDIRECT_URI"),
			auth_url: "https://discord.com/api/oauth2/authorize",
			token_url: "https://discord.com/api/oauth2/token",
			scopes: &["identify", "email"],
		}),
	}
}

/// Generate an authorization URL with PKCE.
///
/// Returns the URL to redirect the user to and the state to store in a cookie.
///
/// # Errors
///
/// Returns an error if the provider is not configured.
pub fn generate_auth_url(provider: OAuthProvider) -> Result<(String, OAuthState), OAuthError> {
	let config = get_provider_config(provider)?;

	let auth_url = AuthUrl::new(config.auth_url.to_string()).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;
	let token_url = TokenUrl::new(config.token_url.to_string()).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;
	let redirect_url = RedirectUrl::new(config.redirect_uri).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;

	let client = oauth2::basic::BasicClient::new(ClientId::new(config.client_id))
		.set_client_secret(ClientSecret::new(config.client_secret))
		.set_auth_uri(auth_url)
		.set_token_uri(token_url)
		.set_redirect_uri(redirect_url);

	let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

	let mut auth_request = client.authorize_url(CsrfToken::new_random).set_pkce_challenge(pkce_challenge);

	for scope in config.scopes {
		auth_request = auth_request.add_scope(Scope::new((*scope).to_string()));
	}

	let (url, csrf_token) = auth_request.url();

	let state = OAuthState {
		csrf_token: csrf_token.secret().clone(),
		pkce_verifier: pkce_verifier.secret().clone(),
		redirect_after: None,
	};

	Ok((url.to_string(), state))
}

/// Exchange an authorization code for an access token.
///
/// # Errors
///
/// Returns an error if the token exchange fails.
pub async fn exchange_code(provider: OAuthProvider, code: &str, state: &OAuthState) -> Result<String, OAuthError> {
	let config = get_provider_config(provider)?;

	let auth_url = AuthUrl::new(config.auth_url.to_string()).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;
	let token_url = TokenUrl::new(config.token_url.to_string()).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;
	let redirect_url = RedirectUrl::new(config.redirect_uri).map_err(|e| OAuthError::ClientBuildError(e.to_string()))?;

	let client = oauth2::basic::BasicClient::new(ClientId::new(config.client_id))
		.set_client_secret(ClientSecret::new(config.client_secret))
		.set_auth_uri(auth_url)
		.set_token_uri(token_url)
		.set_redirect_uri(redirect_url);

	let pkce_verifier = PkceCodeVerifier::new(state.pkce_verifier.clone());

	let http_client = reqwest::Client::builder()
		.redirect(reqwest::redirect::Policy::none())
		.build()
		.map_err(|e| OAuthError::TokenExchangeError(e.to_string()))?;

	let token_result = client
		.exchange_code(AuthorizationCode::new(code.to_string()))
		.set_pkce_verifier(pkce_verifier)
		.request_async(&http_client)
		.await
		.map_err(|e| OAuthError::TokenExchangeError(e.to_string()))?;

	Ok(token_result.access_token().secret().clone())
}

/// Fetch user info from the OAuth provider.
///
/// # Errors
///
/// Returns an error if the user info request fails.
pub async fn fetch_user_info(provider: OAuthProvider, access_token: &str) -> Result<OAuthUserInfo, OAuthError> {
	let client = reqwest::Client::new();

	match provider {
		OAuthProvider::Google => {
			let response = client
				.get("https://www.googleapis.com/oauth2/v2/userinfo")
				.bearer_auth(access_token)
				.send()
				.await
				.map_err(|e| OAuthError::UserInfoError(e.to_string()))?;

			let user_info: GoogleUserInfo = response.json().await.map_err(|e| OAuthError::UserInfoError(e.to_string()))?;

			Ok(user_info.into())
		}
		OAuthProvider::Discord => {
			let response = client
				.get("https://discord.com/api/users/@me")
				.bearer_auth(access_token)
				.send()
				.await
				.map_err(|e| OAuthError::UserInfoError(e.to_string()))?;

			let user_info: DiscordUserInfo = response.json().await.map_err(|e| OAuthError::UserInfoError(e.to_string()))?;

			Ok(user_info.into())
		}
	}
}
