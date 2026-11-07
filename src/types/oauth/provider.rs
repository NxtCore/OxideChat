use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use super::ClaimMapping;
use crate::utils::encryption::{decrypt_api_key, encrypt_secret_value};

const MAX_SLUG_CHARS: usize = 50;

// ============================================================================
// Request Types
// ============================================================================

/// Where the provider's sign-in pages and APIs live. An issuer URL fills in the other three by discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OAuthEndpoints {
	pub issuer_url: Option<String>,
	pub authorization_url: Option<String>,
	pub token_url: Option<String>,
	pub userinfo_url: Option<String>,
}

/// Creates a sign-in provider; an update takes the same shape and ignores the slug.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveOAuthProviderRequest {
	pub slug: String,
	pub name: String,
	pub icon: Option<String>,
	pub endpoints: OAuthEndpoints,
	pub client_id: String,
	/// Required when creating; an update without one keeps the stored secret.
	pub client_secret: Option<String>,
	pub scopes: Vec<String>,
	#[serde(default)]
	pub claim_mapping: ClaimMapping,
	/// Treat the provider's email as verified even when it does not say so.
	#[serde(default)]
	pub trust_email: bool,
	/// Let people without an account create one by signing in with this provider.
	#[serde(default = "default_true")]
	pub allow_registration: bool,
	#[serde(default = "default_true")]
	pub enabled: bool,
}

fn default_true() -> bool {
	true
}

impl SaveOAuthProviderRequest {
	/// Whether the fields are usable; a new provider also needs a slug and a client secret.
	#[must_use]
	pub fn is_valid(&self, creating: bool) -> bool {
		let name_length = self.name.trim().chars().count();
		let has_secret = self.client_secret.as_deref().is_some_and(|secret| !secret.is_empty());
		let icon_fits = self.icon.as_deref().is_none_or(|icon| icon.chars().count() <= 50);
		(1..=100).contains(&name_length) && icon_fits && !self.client_id.trim().is_empty() && (!creating || (is_valid_slug(&self.slug) && has_secret))
	}
}

// ============================================================================
// Response Types
// ============================================================================

/// What the login page needs to show a button.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PublicOAuthProvider {
	/// Used in `/api/v1/auth/oauth/{slug}`.
	pub slug: String,
	pub name: String,
	pub icon: Option<String>,
}

/// A provider as admins see it; the client secret is never returned.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OAuthProviderResponse {
	pub id: Uuid,
	pub slug: String,
	pub name: String,
	pub icon: Option<String>,
	pub endpoints: OAuthEndpoints,
	pub client_id: String,
	pub scopes: Vec<String>,
	pub claim_mapping: ClaimMapping,
	pub trust_email: bool,
	pub allow_registration: bool,
	pub enabled: bool,
	/// The address to register at the provider; needs `BASE_URL`.
	pub callback_url: Option<String>,
}

/// A ready-made starting point for the admin form.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OAuthPresetResponse {
	pub slug: String,
	pub name: String,
	pub icon: Option<String>,
	pub endpoints: OAuthEndpoints,
	pub scopes: Vec<String>,
	pub claim_mapping: ClaimMapping,
}

// ============================================================================
// Internal Types
// ============================================================================

/// Endpoints after discovery: all three are known.
#[derive(Debug, Clone)]
pub struct ResolvedEndpoints {
	pub issuer_url: Option<String>,
	pub authorization_url: String,
	pub token_url: String,
	pub userinfo_url: String,
}

/// A configured sign-in provider.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OAuthProvider {
	pub id: Uuid,
	pub slug: String,
	pub name: String,
	pub icon: Option<String>,
	pub issuer_url: Option<String>,
	pub authorization_url: String,
	pub token_url: String,
	pub userinfo_url: String,
	pub client_id: String,
	pub client_secret: String,
	pub scopes: Vec<String>,
	pub claim_mapping: Json<ClaimMapping>,
	pub trust_email: bool,
	pub allow_registration: bool,
	pub enabled: bool,
}

struct Preset {
	slug: &'static str,
	name: &'static str,
	issuer_url: Option<&'static str>,
	authorization_url: &'static str,
	token_url: &'static str,
	userinfo_url: &'static str,
	scopes: &'static [&'static str],
}

const PRESETS: [Preset; 2] = [
	Preset {
		slug: "google",
		name: "Google",
		issuer_url: Some("https://accounts.google.com"),
		authorization_url: "https://accounts.google.com/o/oauth2/v2/auth",
		token_url: "https://oauth2.googleapis.com/token",
		userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo",
		scopes: &["openid", "email", "profile"],
	},
	Preset {
		slug: "discord",
		name: "Discord",
		issuer_url: None,
		authorization_url: "https://discord.com/api/oauth2/authorize",
		token_url: "https://discord.com/api/oauth2/token",
		userinfo_url: "https://discord.com/api/users/@me",
		scopes: &["identify", "email"],
	},
];

impl Preset {
	fn claim_mapping(&self) -> ClaimMapping {
		match self.slug {
			"discord" => ClaimMapping {
				subject: "id".to_string(),
				email_verified: "verified".to_string(),
				name: "username".to_string(),
				avatar: "avatar".to_string(),
				..ClaimMapping::default()
			},
			_ => ClaimMapping::default(),
		}
	}

	fn to_response(&self) -> OAuthPresetResponse {
		OAuthPresetResponse {
			slug: self.slug.to_string(),
			name: self.name.to_string(),
			icon: Some(self.slug.to_string()),
			endpoints: OAuthEndpoints {
				issuer_url: self.issuer_url.map(str::to_string),
				authorization_url: Some(self.authorization_url.to_string()),
				token_url: Some(self.token_url.to_string()),
				userinfo_url: Some(self.userinfo_url.to_string()),
			},
			scopes: self.scopes.iter().map(|scope| (*scope).to_string()).collect(),
			claim_mapping: self.claim_mapping(),
		}
	}
}

// ============================================================================
// Database access
// ============================================================================

fn protect(secret: &str) -> Result<String, sqlx::Error> {
	encrypt_secret_value(secret, None).map_err(|error| sqlx::Error::Protocol(error.to_string()))
}

/// Whether a slug is short, lowercase and safe to use in a URL.
#[must_use]
pub fn is_valid_slug(slug: &str) -> bool {
	let mut chars = slug.chars();
	chars.next().is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
		&& slug.chars().count() <= MAX_SLUG_CHARS
		&& chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl OAuthProvider {
	/// All ready-made starting points for the admin form.
	#[must_use]
	pub fn presets() -> Vec<OAuthPresetResponse> {
		PRESETS.iter().map(Preset::to_response).collect()
	}

	/// Every provider, for the admin page.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM oauth_providers ORDER BY name").fetch_all(pool).await
	}

	/// Enabled providers, for the login page.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list_enabled(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM oauth_providers WHERE enabled ORDER BY name").fetch_all(pool).await
	}

	/// The enabled provider behind a login URL.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find_enabled_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM oauth_providers WHERE slug = $1 AND enabled")
			.bind(slug)
			.fetch_optional(pool)
			.await
	}

	/// Adds a provider; the slug must be unused.
	///
	/// # Errors
	///
	/// Returns a database error when the insert fails, for example on a duplicate slug.
	pub async fn create(pool: &PgPool, req: &SaveOAuthProviderRequest, endpoints: &ResolvedEndpoints) -> Result<Self, sqlx::Error> {
		let secret = protect(req.client_secret.as_deref().unwrap_or_default())?;
		sqlx::query_as(
			"INSERT INTO oauth_providers
			 (slug, name, icon, issuer_url, authorization_url, token_url, userinfo_url, client_id, client_secret,
			  scopes, claim_mapping, trust_email, allow_registration, enabled)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
			 RETURNING *",
		)
		.bind(&req.slug)
		.bind(&req.name)
		.bind(&req.icon)
		.bind(&endpoints.issuer_url)
		.bind(&endpoints.authorization_url)
		.bind(&endpoints.token_url)
		.bind(&endpoints.userinfo_url)
		.bind(&req.client_id)
		.bind(secret)
		.bind(&req.scopes)
		.bind(Json(&req.claim_mapping))
		.bind(req.trust_email)
		.bind(req.allow_registration)
		.bind(req.enabled)
		.fetch_one(pool)
		.await
	}

	/// Changes a provider; returns `None` when it does not exist.
	///
	/// # Errors
	///
	/// Returns a database error when the update fails.
	pub async fn update(pool: &PgPool, id: Uuid, req: &SaveOAuthProviderRequest, endpoints: &ResolvedEndpoints) -> Result<Option<Self>, sqlx::Error> {
		let secret = req.client_secret.as_deref().filter(|secret| !secret.is_empty()).map(protect).transpose()?;
		sqlx::query_as(
			"UPDATE oauth_providers SET
			 name = $2, icon = $3, issuer_url = $4, authorization_url = $5, token_url = $6, userinfo_url = $7, client_id = $8,
			 client_secret = COALESCE($9, client_secret), scopes = $10, claim_mapping = $11, trust_email = $12,
			 allow_registration = $13, enabled = $14, updated_at = NOW()
			 WHERE id = $1
			 RETURNING *",
		)
		.bind(id)
		.bind(&req.name)
		.bind(&req.icon)
		.bind(&endpoints.issuer_url)
		.bind(&endpoints.authorization_url)
		.bind(&endpoints.token_url)
		.bind(&endpoints.userinfo_url)
		.bind(&req.client_id)
		.bind(secret)
		.bind(&req.scopes)
		.bind(Json(&req.claim_mapping))
		.bind(req.trust_email)
		.bind(req.allow_registration)
		.bind(req.enabled)
		.fetch_optional(pool)
		.await
	}

	/// Removes a provider; returns whether it existed. People who signed in with it keep their accounts.
	///
	/// # Errors
	///
	/// Returns a database error when the delete fails.
	pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM oauth_providers WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

	/// The stored client secret in plain text.
	///
	/// # Errors
	///
	/// Returns an error when the secret cannot be decrypted.
	pub fn plain_client_secret(&self) -> Result<String, crate::utils::encryption::EncryptionError> {
		decrypt_api_key(&self.client_secret)
	}

	/// The address the provider sends people back to, for `base_url` such as `https://chat.example.com`.
	#[must_use]
	pub fn callback_url(&self, base_url: &str) -> String {
		format!("{}/api/v1/auth/oauth/{}/callback", base_url.trim_end_matches('/'), self.slug)
	}

	/// The admin view of this provider.
	#[must_use]
	pub fn to_response(&self) -> OAuthProviderResponse {
		OAuthProviderResponse {
			id: self.id,
			slug: self.slug.clone(),
			name: self.name.clone(),
			icon: self.icon.clone(),
			endpoints: OAuthEndpoints {
				issuer_url: self.issuer_url.clone(),
				authorization_url: Some(self.authorization_url.clone()),
				token_url: Some(self.token_url.clone()),
				userinfo_url: Some(self.userinfo_url.clone()),
			},
			client_id: self.client_id.clone(),
			scopes: self.scopes.clone(),
			claim_mapping: self.claim_mapping.0.clone(),
			trust_email: self.trust_email,
			allow_registration: self.allow_registration,
			enabled: self.enabled,
			callback_url: std::env::var("BASE_URL").ok().map(|base_url| self.callback_url(&base_url)),
		}
	}

	/// The login page view of this provider.
	#[must_use]
	pub fn to_public(&self) -> PublicOAuthProvider {
		PublicOAuthProvider {
			slug: self.slug.clone(),
			name: self.name.clone(),
			icon: self.icon.clone(),
		}
	}
}
