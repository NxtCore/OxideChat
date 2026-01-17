//! Internationalization (i18n) module for OxideChat.
//!
//! Provides a global translation service that:
//! - Loads translations from database once at startup
//! - Can be accessed globally via `I18n::get()`
//! - Supports reloading translations at runtime
//! - Template variable interpolation

use arc_swap::ArcSwap;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::config::Config;

/// Global i18n instance
static I18N: OnceLock<I18n> = OnceLock::new();

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
	En,
	De,
}

impl Language {
	/// Convert from string, defaulting to English if unknown
	#[must_use]
	pub fn from_str(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"de" | "german" | "deutsch" => Self::De,
			_ => Self::En,
		}
	}

	/// Get the language code as a string
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::En => "en",
			Self::De => "de",
		}
	}
}

impl Default for Language {
	fn default() -> Self {
		Self::En
	}
}

/// Row from i18n_translations table
#[derive(sqlx::FromRow)]
struct TranslationRow {
	language: String,
	key_path: String,
	value: String,
}

/// Global translation service.
///
/// Initialize once at startup with `I18n::init()`, then access via `I18n::get()`.
pub struct I18n {
	translations: ArcSwap<Value>,
}

// Manual Debug impl since RwLock<Value> doesn't derive Debug nicely
impl std::fmt::Debug for I18n {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("I18n").finish()
	}
}

impl I18n {
	/// Initialize the global i18n instance.
	///
	/// Call this once at startup after database connection is established.
	/// Panics if called more than once.
	pub async fn init(pool: &PgPool) {
		let translations = Self::load_from_db(pool).await;
		let instance = Self {
			translations: ArcSwap::new(Arc::new(translations)),
		};
		I18N.set(instance).expect("I18n already initialized");
	}

	/// Get the global i18n instance.
	///
	/// Panics if `init()` hasn't been called yet.
	#[must_use]
	pub fn get() -> &'static Self {
		I18N.get().expect("I18n not initialized. Call I18n::init() at startup.")
	}

	/// Reload translations from the database.
	///
	/// Use this when translations have been modified via admin endpoints.
	pub async fn reload(&self, pool: &PgPool) {
		let translations = Self::load_from_db(pool).await;
		self.translations.store(Arc::new(translations));
	}

	/// Get a translation by key path with optional variable interpolation.
	///
	/// # Arguments
	/// * `key` - Dot-separated key path (e.g., "common.save")
	/// * `language` - Target language
	/// * `args` - Template variables to interpolate (e.g., `{name}` -> value)
	///
	/// # Returns
	/// The translated string, or the key if not found.
	#[must_use]
	pub fn translate(&self, key: &str, args: &Option<HashMap<String, String>>) -> String {
		let lock = self.translations.load();
		let lang_key = Config::get().language().as_str();

		let mut current: &Value = match lock.get(lang_key) {
			Some(v) => v,
			None => return key.to_string(),
		};

		for part in key.split('.') {
			match current.get(part) {
				Some(v) => current = v,
				None => return key.to_string(),
			}
		}

		match current.as_str() {
			Some(s) => match args {
				Some(args) => Self::interpolate(s, args),
				None => s.to_string(),
			},
			None => key.to_string(),
		}
	}

	/// Get all translations as JSON (for the /base endpoint).
	#[must_use]
	pub fn all(&self) -> Arc<Value> {
		self.translations.load_full()
	}

	/// Load translations from database.
	async fn load_from_db(pool: &PgPool) -> Value {
		let rows: Vec<TranslationRow> = sqlx::query_as("SELECT language, key_path, value FROM i18n_translations ORDER BY language, key_path")
			.fetch_all(pool)
			.await
			.unwrap_or_default();

		let mut translations = serde_json::json!({
			"en": {},
			"de": {}
		});

		for row in rows {
			if let Some(lang_obj) = translations.get_mut(&row.language) {
				Self::set_nested_value(lang_obj, &row.key_path, &row.value);
			}
		}

		translations
	}

	/// Interpolate template variables in a string.
	fn interpolate(template: &str, args: &HashMap<String, String>) -> String {
		if args.is_empty() {
			return template.to_string();
		}

		let mut result = template.to_string();
		for (key, value) in args {
			let pattern = format!("{{{key}}}");
			result = result.replace(&pattern, value);
		}
		result
	}

	/// Set a value in a nested JSON structure using dot-notation path.
	fn set_nested_value(obj: &mut Value, path: &str, value: &str) {
		let parts: Vec<&str> = path.split('.').collect();

		// Handle single-part paths
		if parts.len() == 1 {
			if let Some(map) = obj.as_object_mut() {
				map.insert(parts[0].to_string(), Value::String(value.to_string()));
			}
			return;
		}

		// Navigate to parent, creating intermediate objects as needed
		let mut current = obj;
		for part in &parts[..parts.len() - 1] {
			// Ensure the intermediate object exists
			if !current.get(*part).map_or(false, Value::is_object) {
				if let Some(map) = current.as_object_mut() {
					map.insert((*part).to_string(), serde_json::json!({}));
				}
			}
			if let Some(next) = current.get_mut(*part) {
				current = next;
			} else {
				return;
			}
		}

		// Set the final value
		if let Some(map) = current.as_object_mut() {
			let last_part = parts[parts.len() - 1];
			map.insert(last_part.to_string(), Value::String(value.to_string()));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_language_from_str() {
		assert_eq!(Language::from_str("en"), Language::En);
		assert_eq!(Language::from_str("de"), Language::De);
		assert_eq!(Language::from_str("unknown"), Language::En);
	}

	#[test]
	fn test_interpolation() {
		let mut args = HashMap::new();
		args.insert("name".to_string(), "World".to_string());

		let result = I18n::interpolate("Hello, {name}!", &args);
		assert_eq!(result, "Hello, World!");
	}

	#[test]
	fn test_set_nested_value() {
		let mut obj = serde_json::json!({});
		I18n::set_nested_value(&mut obj, "common.save", "Save");
		assert_eq!(obj["common"]["save"], "Save");
	}
}
