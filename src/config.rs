//! Global application configuration for OxideChat.
//!
//! Provides a global configuration store that:
//! - Loads configuration from database once at startup
//! - Can be accessed globally via `Config::get()`
//! - Supports reloading configuration at runtime

use crate::i18n::Language;
use arc_swap::ArcSwap;
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};

/// Global config instance
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Supported OAuth providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
	Google,
	Apple,
	Discord,
}

impl OAuthProvider {
	/// Convert from string, returns None if unknown
	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s.to_lowercase().as_str() {
			"google" => Some(Self::Google),
			"apple" => Some(Self::Apple),
			"discord" => Some(Self::Discord),
			_ => None,
		}
	}

	/// Get the provider name as a string
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::Google => "google",
			Self::Apple => "apple",
			Self::Discord => "discord",
		}
	}

	/// Get all available OAuth providers
	#[must_use]
	pub const fn all() -> &'static [OAuthProvider] {
		&[Self::Google, Self::Apple, Self::Discord]
	}
}

/// Database row for application configuration
#[derive(sqlx::FromRow)]
struct ConfigRow {
	key: String,
	value: String,
}

/// Configuration values loaded from database
#[derive(Debug, Clone)]
pub struct ConfigValues {
	/// Default language for the application
	pub language: Language,

	// OAuth Google configuration
	pub oauth_google_client_id: Option<String>,
	pub oauth_google_client_secret: Option<String>,
	pub oauth_google_redirect_uri: Option<String>,

	// OAuth Apple configuration
	pub oauth_apple_client_id: Option<String>,
	pub oauth_apple_client_secret: Option<String>,
	pub oauth_apple_redirect_uri: Option<String>,

	// OAuth Discord configuration
	pub oauth_discord_client_id: Option<String>,
	pub oauth_discord_client_secret: Option<String>,
	pub oauth_discord_redirect_uri: Option<String>,
}

impl Default for ConfigValues {
	fn default() -> Self {
		Self {
			language: Language::En,
			oauth_google_client_id: None,
			oauth_google_client_secret: None,
			oauth_google_redirect_uri: None,
			oauth_apple_client_id: None,
			oauth_apple_client_secret: None,
			oauth_apple_redirect_uri: None,
			oauth_discord_client_id: None,
			oauth_discord_client_secret: None,
			oauth_discord_redirect_uri: None,
		}
	}
}

/// Global configuration service.
///
/// Initialize once at startup with `Config::init()`, then access via `Config::get()`.
pub struct Config {
	values: ArcSwap<ConfigValues>,
}

// Manual Debug impl since RwLock doesn't derive Debug nicely
impl std::fmt::Debug for Config {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Config").finish()
	}
}

impl Config {
	/// Initialize the global config instance.
	///
	/// Call this once at startup after database connection is established.
	///
	/// # Panics
	/// Panics if called more than once.
	pub async fn init(pool: &PgPool) {
		let values = Self::load_from_db(pool).await;
		let instance = Self {
			values: ArcSwap::new(Arc::new(values)),
		};
		CONFIG.set(instance).expect("Config already initialized");
	}

	/// Reload configuration from the database.
	///
	/// Use this when configuration has been modified via admin endpoints.
	pub async fn reload(&self, pool: &PgPool) {
		let values = Self::load_from_db(pool).await;
		self.values.store(Arc::new(values));
	}

	/// Get the global config instance.
	///
	/// Panics if `init()` hasn't been called yet.
	#[must_use]
	pub fn get() -> &'static Self {
		CONFIG.get().expect("Config not initialized. Call Config::init() at startup.")
	}

	/// Get the default language.
	#[must_use]
	pub fn language(&self) -> Language {
		self.values.load().language
	}

	/// Check if an OAuth provider is fully configured.
	///
	/// Checks both database config and environment variables (as fallback).
	/// A provider is considered configured if all required fields are set.
	#[must_use]
	pub fn is_oauth_provider_configured(&self, provider: OAuthProvider) -> bool {
		let values = self.values.load();

		match provider {
			OAuthProvider::Google => {
				let has_client_id = values.oauth_google_client_id.is_some() || std::env::var("OAUTH_GOOGLE_CLIENT_ID").ok().filter(|s| !s.is_empty()).is_some();
				let has_client_secret =
					values.oauth_google_client_secret.is_some() || std::env::var("OAUTH_GOOGLE_CLIENT_SECRET").ok().filter(|s| !s.is_empty()).is_some();
				let has_redirect_uri = values.oauth_google_redirect_uri.is_some() || std::env::var("OAUTH_GOOGLE_REDIRECT_URI").ok().filter(|s| !s.is_empty()).is_some();

				has_client_id && has_client_secret && has_redirect_uri
			}
			OAuthProvider::Apple => {
				let has_client_id = values.oauth_apple_client_id.is_some() || std::env::var("OAUTH_APPLE_CLIENT_ID").ok().filter(|s| !s.is_empty()).is_some();
				let has_client_secret = values.oauth_apple_client_secret.is_some() || std::env::var("OAUTH_APPLE_CLIENT_SECRET").ok().filter(|s| !s.is_empty()).is_some();
				let has_redirect_uri = values.oauth_apple_redirect_uri.is_some() || std::env::var("OAUTH_APPLE_REDIRECT_URI").ok().filter(|s| !s.is_empty()).is_some();

				has_client_id && has_client_secret && has_redirect_uri
			}
			OAuthProvider::Discord => {
				let has_client_id = values.oauth_discord_client_id.is_some() || std::env::var("OAUTH_DISCORD_CLIENT_ID").ok().filter(|s| !s.is_empty()).is_some();
				let has_client_secret =
					values.oauth_discord_client_secret.is_some() || std::env::var("OAUTH_DISCORD_CLIENT_SECRET").ok().filter(|s| !s.is_empty()).is_some();
				let has_redirect_uri =
					values.oauth_discord_redirect_uri.is_some() || std::env::var("OAUTH_DISCORD_REDIRECT_URI").ok().filter(|s| !s.is_empty()).is_some();

				has_client_id && has_client_secret && has_redirect_uri
			}
		}
	}

	/// Get a list of configured OAuth providers.
	///
	/// Returns a vector of configured OAuth provider enums.
	#[must_use]
	pub fn get_configured_oauth_providers(&self) -> Vec<OAuthProvider> {
		OAuthProvider::all()
			.iter()
			.copied()
			.filter(|&provider| self.is_oauth_provider_configured(provider))
			.collect()
	}

	/// Load configuration from database.
	async fn load_from_db(pool: &PgPool) -> ConfigValues {
		let rows: Vec<ConfigRow> = sqlx::query_as("SELECT key, value FROM app_config").fetch_all(pool).await.unwrap_or_default();

		let mut values = ConfigValues::default();

		for row in rows {
			match row.key.as_str() {
				"language" => values.language = Language::from_str(&row.value),
				"oauth_google_client_id" => values.oauth_google_client_id = Some(row.value),
				"oauth_google_client_secret" => values.oauth_google_client_secret = Some(row.value),
				"oauth_google_redirect_uri" => values.oauth_google_redirect_uri = Some(row.value),
				"oauth_apple_client_id" => values.oauth_apple_client_id = Some(row.value),
				"oauth_apple_client_secret" => values.oauth_apple_client_secret = Some(row.value),
				"oauth_apple_redirect_uri" => values.oauth_apple_redirect_uri = Some(row.value),
				"oauth_discord_client_id" => values.oauth_discord_client_id = Some(row.value),
				"oauth_discord_client_secret" => values.oauth_discord_client_secret = Some(row.value),
				"oauth_discord_redirect_uri" => values.oauth_discord_redirect_uri = Some(row.value),
				_ => {} // Ignore unknown config keys for forward compatibility
			}
		}

		values
	}
}
