//! Global application configuration for `OxideChat`.
//!
//! Provides a global configuration store that:
//! - Loads configuration from database once at startup
//! - Can be accessed globally via `Config::get()`
//! - Supports reloading configuration at runtime

use crate::i18n::Language;
use crate::types::{ThemeCssVars, TitleGeneration, UpdateGlobalConfigRequest};
use arc_swap::ArcSwap;
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

/// Global config instance
static CONFIG: OnceLock<Config> = OnceLock::new();

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
	/// When enabled, the chat UI exposes a per-message upstream-provider selector
	/// (`OpenRouter` provider routing). Off by default.
	pub enable_provider_selector: bool,
	/// Instance-wide default model. Applied when neither user nor team default is set.
	pub default_model_id: Option<Uuid>,
	/// Instance name shown instead of `OxideChat`.
	pub instance_name: Option<String>,
	/// Image shown as the favicon and app icon instead of the `OxideChat` logo.
	pub instance_icon_id: Option<Uuid>,
	/// Generated chat titles; off by default.
	pub title_generation: TitleGeneration,
}

impl Default for ConfigValues {
	fn default() -> Self {
		Self {
			language: Language::En,
			default_theme: ThemeCssVars::default(),
			enable_provider_selector: false,
			default_model_id: None,
			instance_name: None,
			instance_icon_id: None,
			title_generation: TitleGeneration::default(),
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

	/// Get the instance-wide default model ID, if configured.
	#[must_use]
	pub fn default_model_id(&self) -> Option<Uuid> {
		self.values.load().default_model_id
	}

	/// Get the instance name, if an admin set one.
	#[must_use]
	pub fn instance_name(&self) -> Option<String> {
		self.values.load().instance_name.clone()
	}

	/// Get the image ID of the instance icon, if an admin set one.
	#[must_use]
	pub fn instance_icon_id(&self) -> Option<Uuid> {
		self.values.load().instance_icon_id
	}

	/// Get the chat title generation setting.
	#[must_use]
	pub fn title_generation(&self) -> TitleGeneration {
		self.values.load().title_generation
	}

	/// Store the given settings in one transaction and reload config.
	///
	/// # Errors
	/// Returns an error if serialization or a database write fails; nothing is changed then.
	pub async fn apply_update(&self, pool: &PgPool, req: &UpdateGlobalConfigRequest) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		Self::store_settings(&mut tx, req).await?;
		Self::store_title_generation(&mut tx, req.title_generation.as_ref()).await?;
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
		if let Some(language) = req.language {
			Self::upsert_config(&mut **tx, "language", language.as_str()).await?;
		}
		if let Some(instance) = &req.instance {
			Self::set_or_clear(tx, "instance_name", instance.name.as_ref()).await?;
			let icon_id = instance.icon_id.map(|id| id.map(|id| id.to_string()));
			Self::set_or_clear(tx, "instance_icon_id", icon_id.as_ref()).await?;
		}
		let default_model_id = req.default_model_id.map(|id| id.map(|id| id.to_string()));
		Self::set_or_clear(tx, "default_model_id", default_model_id.as_ref()).await
	}

	async fn store_title_generation(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, title: Option<&TitleGeneration>) -> Result<(), sqlx::Error> {
		let Some(title) = title else {
			return Ok(());
		};
		Self::upsert_config(&mut **tx, "title_generation_enabled", if title.enabled { "true" } else { "false" }).await?;
		Self::set_or_clear(tx, "title_model_id", Some(&title.model_id.map(|id| id.to_string()))).await
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
				"enable_provider_selector" => values.enable_provider_selector = row.value == "true",
				"default_model_id" => values.default_model_id = Uuid::parse_str(&row.value).ok(),
				"instance_name" => values.instance_name = Some(row.value),
				"instance_icon_id" => values.instance_icon_id = Uuid::parse_str(&row.value).ok(),
				"title_generation_enabled" => values.title_generation.enabled = row.value == "true",
				"title_model_id" => values.title_generation.model_id = Uuid::parse_str(&row.value).ok(),
				_ => {} // Ignore unknown config keys for forward compatibility
			}
		}

		values
	}
}
