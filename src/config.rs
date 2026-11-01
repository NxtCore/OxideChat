//! Global application configuration for `OxideChat`.
//!
//! Provides a global configuration store that:
//! - Loads configuration from database once at startup
//! - Can be accessed globally via `Config::get()`
//! - Supports reloading configuration at runtime

use crate::i18n::Language;
use crate::types::{OAuthSettingsResponse, ThemeCssVars, UpdateGlobalConfigRequest};
use crate::utils::encryption::{decrypt_api_key, encrypt_secret_value};
use arc_swap::ArcSwap;
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

/// Global config instance
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Supported OAuth providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum OAuthProvider {
	Google,
	Discord,
}

impl OAuthProvider {
	/// Convert from string, returns None if unknown
	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s.to_uppercase().as_str() {
			"GOOGLE" => Some(Self::Google),
			"DISCORD" => Some(Self::Discord),
			_ => None,
		}
	}

	/// Get the provider name as a string
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::Google => "GOOGLE",
			Self::Discord => "DISCORD",
		}
	}

	/// Get all available OAuth providers
	#[must_use]
	pub const fn all() -> &'static [OAuthProvider] {
		&[Self::Google, Self::Discord]
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
	pub language: Language,
	pub default_theme: ThemeCssVars,
	pub oauth_google_client_id: Option<String>,
	pub oauth_google_client_secret: Option<String>,
	pub oauth_discord_client_id: Option<String>,
	pub oauth_discord_client_secret: Option<String>,
	/// When enabled, the chat UI exposes a per-message upstream-provider selector
	/// (`OpenRouter` provider routing). Off by default.
	pub enable_provider_selector: bool,
	/// When enabled, admins may register global MCP servers that run over
	/// server-side `stdio` (arbitrary commands on the host). Off by default.
	pub allow_server_stdio_mcp: bool,
	/// Instance-wide default model. Applied when neither user nor team default is set.
	pub default_model_id: Option<Uuid>,
}

impl Default for ConfigValues {
	fn default() -> Self {
		Self {
			language: Language::En,
			default_theme: ThemeCssVars::default(),
			oauth_google_client_id: None,
			oauth_google_client_secret: None,
			oauth_discord_client_id: None,
			oauth_discord_client_secret: None,
			enable_provider_selector: false,
			allow_server_stdio_mcp: false,
			default_model_id: None,
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
	/// Later calls are ignored; the first initialization wins.
	pub async fn init(pool: &PgPool) {
		let values = Self::load_from_db(pool).await;
		let instance = Self {
			values: ArcSwap::new(Arc::new(values)),
		};
		let _ = CONFIG.set(instance);
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

	/// Get the default theme.
	#[must_use]
	pub fn default_theme(&self) -> ThemeCssVars {
		self.values.load().default_theme.clone()
	}

	/// Whether the chat provider selector is enabled instance-wide.
	#[must_use]
	pub fn enable_provider_selector(&self) -> bool {
		self.values.load().enable_provider_selector
	}

	/// Whether admins may register server-side `stdio` MCP servers.
	#[must_use]
	pub fn allow_server_stdio_mcp(&self) -> bool {
		self.values.load().allow_server_stdio_mcp
	}

	/// Get the instance-wide default model ID, if configured.
	#[must_use]
	pub fn default_model_id(&self) -> Option<Uuid> {
		self.values.load().default_model_id
	}

	/// Get current configuration values.
	#[must_use]
	pub fn values(&self) -> std::sync::Arc<ConfigValues> {
		self.values.load_full()
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

				has_client_id && has_client_secret
			}
			OAuthProvider::Discord => {
				let has_client_id = values.oauth_discord_client_id.is_some() || std::env::var("OAUTH_DISCORD_CLIENT_ID").ok().filter(|s| !s.is_empty()).is_some();
				let has_client_secret =
					values.oauth_discord_client_secret.is_some() || std::env::var("OAUTH_DISCORD_CLIENT_SECRET").ok().filter(|s| !s.is_empty()).is_some();

				has_client_id && has_client_secret
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

	/// Describe each OAuth provider for the admin page. Secrets are only reported as present or not.
	#[must_use]
	pub fn oauth_settings(&self) -> Vec<OAuthSettingsResponse> {
		let values = self.values.load();
		OAuthProvider::all()
			.iter()
			.map(|&provider| {
				let (client_id, client_secret, env_id, env_secret) = match provider {
					OAuthProvider::Google => (&values.oauth_google_client_id, &values.oauth_google_client_secret, "OAUTH_GOOGLE_CLIENT_ID", "OAUTH_GOOGLE_CLIENT_SECRET"),
					OAuthProvider::Discord => (&values.oauth_discord_client_id, &values.oauth_discord_client_secret, "OAUTH_DISCORD_CLIENT_ID", "OAUTH_DISCORD_CLIENT_SECRET"),
				};
				OAuthSettingsResponse {
					provider,
					client_id: client_id.clone(),
					has_client_secret: client_secret.is_some(),
					env_configured: Self::env_is_set(env_id) && Self::env_is_set(env_secret),
					configured: self.is_oauth_provider_configured(provider),
					callback_url: std::env::var("BASE_URL").ok().map(|base_url| format!("{base_url}/api/v1/auth/oauth/{}/callback", provider.as_str())),
				}
			})
			.collect()
	}

	fn env_is_set(key: &str) -> bool {
		std::env::var(key).is_ok_and(|value| !value.is_empty())
	}

	/// Store the given settings in one transaction and reload config.
	///
	/// # Errors
	/// Returns an error if serialization or a database write fails; nothing is changed then.
	pub async fn apply_update(&self, pool: &PgPool, req: &UpdateGlobalConfigRequest) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		Self::store_settings(&mut tx, req).await?;
		for update in &req.oauth {
			let prefix = format!("oauth_{}", update.provider.as_str().to_lowercase());
			Self::set_or_clear(&mut tx, &format!("{prefix}_client_id"), update.client_id.as_ref()).await?;
			Self::set_or_clear(&mut tx, &format!("{prefix}_client_secret"), update.client_secret.as_ref()).await?;
		}
		tx.commit().await?;
		self.reload(pool).await;
		Ok(())
	}

	async fn store_settings(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, req: &UpdateGlobalConfigRequest) -> Result<(), sqlx::Error> {
		if let Some(theme) = &req.default_theme {
			let json = serde_json::to_string(theme).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
			Self::upsert_config(&mut **tx, "default_theme", &json).await?;
		}
		if let Some(value) = req.enable_provider_selector {
			Self::upsert_config(&mut **tx, "enable_provider_selector", if value { "true" } else { "false" }).await?;
		}
		if let Some(value) = req.allow_server_stdio_mcp {
			Self::upsert_config(&mut **tx, "allow_server_stdio_mcp", if value { "true" } else { "false" }).await?;
		}
		if let Some(language) = req.language {
			Self::upsert_config(&mut **tx, "language", language.as_str()).await?;
		}
		let default_model_id = req.default_model_id.map(|id| id.map(|id| id.to_string()));
		Self::set_or_clear(tx, "default_model_id", default_model_id.as_ref()).await
	}

	async fn set_or_clear(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, key: &str, value: Option<&Option<String>>) -> Result<(), sqlx::Error> {
		match value {
			None => Ok(()),
			Some(Some(value)) if !value.trim().is_empty() => Self::upsert_config(&mut **tx, key, value.trim()).await,
			Some(_) => {
				sqlx::query("DELETE FROM app_config WHERE key = $1").bind(key).execute(&mut **tx).await?;
				Ok(())
			}
		}
	}

	async fn upsert_config<'e, E: sqlx::PgExecutor<'e>>(executor: E, key: &str, value: &str) -> Result<(), sqlx::Error> {
		let protected;
		let value = if matches!(key, "oauth_google_client_secret" | "oauth_discord_client_secret") {
			protected = encrypt_secret_value(value, None).map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
			protected.as_str()
		} else {
			value
		};
		sqlx::query(
			r#"
			INSERT INTO app_config (key, value)
			VALUES ($1, $2)
			ON CONFLICT (key) DO UPDATE SET value = $2
			"#,
		)
		.bind(key)
		.bind(value)
		.execute(executor)
		.await?;
		Ok(())
	}

	/// Load configuration from database.
	#[allow(clippy::cognitive_complexity)]
	async fn load_from_db(pool: &PgPool) -> ConfigValues {
		let rows: Vec<ConfigRow> = sqlx::query_as("SELECT key, value FROM app_config").fetch_all(pool).await.unwrap_or_default();

		let mut values = ConfigValues::default();

		for row in rows {
			match row.key.as_str() {
				"language" => values.language = Language::from_str(&row.value),
				"default_theme" => {
					if let Ok(theme) = serde_json::from_str(&row.value) {
						values.default_theme = theme;
					}
				}
				"oauth_google_client_id" => values.oauth_google_client_id = Some(row.value),
				"oauth_google_client_secret" => match decrypt_api_key(&row.value) {
					Ok(value) => values.oauth_google_client_secret = Some(value),
					Err(error) => tracing::error!("Failed to decrypt Google OAuth client secret: {error}"),
				},
				"oauth_discord_client_id" => values.oauth_discord_client_id = Some(row.value),
				"oauth_discord_client_secret" => match decrypt_api_key(&row.value) {
					Ok(value) => values.oauth_discord_client_secret = Some(value),
					Err(error) => tracing::error!("Failed to decrypt Discord OAuth client secret: {error}"),
				},
				"enable_provider_selector" => values.enable_provider_selector = row.value == "true",
				"allow_server_stdio_mcp" => values.allow_server_stdio_mcp = row.value == "true",
				"default_model_id" => values.default_model_id = Uuid::parse_str(&row.value).ok(),
				_ => {} // Ignore unknown config keys for forward compatibility
			}
		}

		values
	}
}
