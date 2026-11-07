//! OAuth service for sign-in providers.
//!
//! Builds authorization URLs with PKCE, exchanges codes for tokens, reads the user's profile through the
//! provider's claim mapping and finds endpoints by OIDC discovery.

use std::env;

use crate::types::oauth::{OAuthEndpoints, OAuthProvider, OAuthState, OAuthUserInfo, ResolvedEndpoints};
use crate::utils::outbound::{self, OutboundError};
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl};
use serde::Deserialize;

type ProviderClient = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// OAuth-related errors.
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
	#[error("Failed to build OAuth client: {0}")]
	ClientBuild(String),
	#[error("Failed to exchange code: {0}")]
	TokenExchange(String),
	#[error("Failed to fetch user info: {0}")]
	UserInfo(String),
	#[error("Failed to discover the provider's endpoints: {0}")]
	Discovery(String),
	#[error("BASE_URL environment variable must be set for OAuth redirect URL")]
	MissingBaseUrl,
	#[error(transparent)]
	Outbound(#[from] OutboundError),
}

#[derive(Deserialize)]
struct DiscoveryDocument {
	issuer: String,
	authorization_endpoint: Option<String>,
	token_endpoint: Option<String>,
	userinfo_endpoint: Option<String>,
}

fn build_client(provider: &OAuthProvider) -> Result<ProviderClient, OAuthError> {
	let secret = provider.plain_client_secret().map_err(|e| OAuthError::ClientBuild(e.to_string()))?;
	let auth_url = AuthUrl::new(provider.authorization_url.clone()).map_err(|e| OAuthError::ClientBuild(e.to_string()))?;
	let token_url = TokenUrl::new(provider.token_url.clone()).map_err(|e| OAuthError::ClientBuild(e.to_string()))?;
	let base_url = env::var("BASE_URL").map_err(|_| OAuthError::MissingBaseUrl)?;
	let redirect_url = RedirectUrl::new(provider.callback_url(&base_url)).map_err(|e| OAuthError::ClientBuild(e.to_string()))?;

	Ok(BasicClient::new(ClientId::new(provider.client_id.clone()))
		.set_client_secret(ClientSecret::new(secret))
		.set_auth_uri(auth_url)
		.set_token_uri(token_url)
		.set_redirect_uri(redirect_url))
}

/// Generate an authorization URL with PKCE.
///
/// Returns the URL to redirect the user to and the state to store in a cookie.
///
/// # Errors
///
/// Returns an error if the provider's settings cannot build a client.
pub fn generate_auth_url(provider: &OAuthProvider) -> Result<(String, OAuthState), OAuthError> {
	let client = build_client(provider)?;
	let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

	let mut auth_request = client.authorize_url(CsrfToken::new_random).set_pkce_challenge(pkce_challenge);
	for scope in &provider.scopes {
		auth_request = auth_request.add_scope(Scope::new(scope.clone()));
	}
	let (url, csrf_token) = auth_request.url();

	let state = OAuthState {
		csrf_token: csrf_token.secret().clone(),
		pkce_verifier: pkce_verifier.secret().clone(),
		provider_slug: provider.slug.clone(),
		redirect_after: None,
	};
	Ok((url.to_string(), state))
}

/// Exchange an authorization code for an access token.
///
/// # Errors
///
/// Returns an error if the token endpoint is not allowed or the exchange fails.
pub async fn exchange_code(provider: &OAuthProvider, code: &str, state: &OAuthState) -> Result<String, OAuthError> {
	let client = build_client(provider)?;
	let (http_client, _) = outbound::guarded_client(&provider.token_url).await?;

	let token_result = client
		.exchange_code(AuthorizationCode::new(code.to_string()))
		.set_pkce_verifier(PkceCodeVerifier::new(state.pkce_verifier.clone()))
		.request_async(&http_client)
		.await
		.map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

	Ok(token_result.access_token().secret().clone())
}

/// Fetch the user's profile from the provider and read it through the claim mapping.
///
/// # Errors
///
/// Returns an error if the user info endpoint is not allowed, the request fails or the profile has no subject.
pub async fn fetch_user_info(provider: &OAuthProvider, access_token: &str) -> Result<OAuthUserInfo, OAuthError> {
	let (client, url) = outbound::guarded_client(&provider.userinfo_url).await?;
	let response = client
		.get(url)
		.bearer_auth(access_token)
		.send()
		.await
		.and_then(reqwest::Response::error_for_status)
		.map_err(|e| OAuthError::UserInfo(e.to_string()))?;
	let profile: serde_json::Value = response.json().await.map_err(|e| OAuthError::UserInfo(e.to_string()))?;

	OAuthUserInfo::from_profile(&provider.slug, &provider.claim_mapping, &profile).ok_or_else(|| OAuthError::UserInfo("the profile has no subject".to_string()))
}

async fn discover(issuer_url: &str) -> Result<DiscoveryDocument, OAuthError> {
	let issuer = issuer_url.trim_end_matches('/');
	let (client, url) = outbound::guarded_client(&format!("{issuer}/.well-known/openid-configuration")).await?;
	let document: DiscoveryDocument = client
		.get(url)
		.send()
		.await
		.and_then(reqwest::Response::error_for_status)
		.map_err(|e| OAuthError::Discovery(e.to_string()))?
		.json()
		.await
		.map_err(|e| OAuthError::Discovery(e.to_string()))?;
	if document.issuer.trim_end_matches('/') != issuer {
		return Err(OAuthError::Discovery("the issuer in the discovery document does not match".to_string()));
	}
	Ok(document)
}

fn present(value: &Option<String>) -> Option<String> {
	value.as_ref().map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

/// Completes a provider's endpoints: explicit URLs win, missing ones come from OIDC discovery of the issuer.
///
/// # Errors
///
/// Returns an error if an endpoint is still missing, discovery fails, or a URL is not allowed.
pub async fn resolve_endpoints(endpoints: &OAuthEndpoints) -> Result<ResolvedEndpoints, OAuthError> {
	let issuer_url = present(&endpoints.issuer_url);
	let mut authorization_url = present(&endpoints.authorization_url);
	let mut token_url = present(&endpoints.token_url);
	let mut userinfo_url = present(&endpoints.userinfo_url);

	if let Some(issuer) = &issuer_url
		&& (authorization_url.is_none() || token_url.is_none() || userinfo_url.is_none())
	{
		let document = discover(issuer).await?;
		authorization_url = authorization_url.or(document.authorization_endpoint);
		token_url = token_url.or(document.token_endpoint);
		userinfo_url = userinfo_url.or(document.userinfo_endpoint);
	}

	let missing = |name: &str| OAuthError::Discovery(format!("no {name} endpoint"));
	let resolved = ResolvedEndpoints {
		issuer_url,
		authorization_url: authorization_url.ok_or_else(|| missing("authorization"))?,
		token_url: token_url.ok_or_else(|| missing("token"))?,
		userinfo_url: userinfo_url.ok_or_else(|| missing("userinfo"))?,
	};
	for url in [&resolved.authorization_url, &resolved.token_url, &resolved.userinfo_url] {
		outbound::guarded_client(url).await?;
	}
	Ok(resolved)
}
