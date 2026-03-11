use crate::types::BaseType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelConfig {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub model_id: Uuid,
	pub stable_key: String,
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub capabilities: Option<Json<Vec<String>>>,
	pub input_modalities: Option<Json<Vec<String>>>,
	pub output_modalities: Option<Json<Vec<String>>>,
	pub context_length: Option<i32>,
	pub max_output_tokens: Option<i32>,
	pub system_prompt: Option<String>,
	pub sampling: Json<Value>,
	pub enabled_tools: Json<Vec<String>>,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub is_favorite: bool,
	pub category: Option<String>,
	pub tags: Json<Vec<String>>,
	pub usage_count: i32,
	pub extra_settings: Json<Value>,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for ModelConfig {
	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			owner_id: Option::None,
			model_id: Uuid::new_v4(),
			stable_key: "".to_string(),
			name: "".to_string(),
			description: None,
			icon: None,
			capabilities: None,
			input_modalities: None,
			output_modalities: None,
			context_length: None,
			max_output_tokens: None,
			system_prompt: None,
			sampling: Json(Value::Object(serde_json::Map::new())),
			enabled_tools: Json(vec![]),
			is_public: false,
			is_featured: false,
			is_default: false,
			is_favorite: false,
			category: None,
			tags: Json(vec![]),
			usage_count: 0,
			extra_settings: Json(Value::Object(serde_json::Map::new())),
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}
	fn table(&self) -> &str {
		"model_configs"
	}
	fn alias(&self) -> &str {
		"mc"
	}
	fn sql_fields(&self) -> Vec<&str> {
		vec![
			"id",
			"owner_id",
			"model_id",
			"stable_key",
			"name",
			"description",
			"icon",
			"capabilities",
			"input_modalities",
			"output_modalities",
			"context_length",
			"max_output_tokens",
			"system_prompt",
			"sampling",
			"enabled_tools",
			"is_public",
			"is_featured",
			"is_default",
			"is_favorite",
			"category",
			"tags",
			"usage_count",
			"extra_settings",
			"created_at",
			"updated_at",
		]
	}
}

/// A typed value that can be written into a `model_configs` column.
///
/// Each variant maps to the Postgres type of the column it targets.
/// Use `Null` to explicitly clear a nullable column.
/// `JsonMerge` merges a partial JSONB patch into an existing column, removing
/// any keys whose value is JSON null via `jsonb_strip_nulls`.
pub enum ConfigValue<'a> {
	Text(Option<&'a str>),
	Int(Option<i32>),
	Bool(bool),
	Json(Option<&'a Value>),
	JsonMerge(&'a Value),
}

impl ConfigValue<'_> {
	/// Returns the SQL expression fragment for the SET clause.
	///
	/// For most variants this is `$N<cast>`; for `JsonMerge` it is the merge
	/// expression `jsonb_strip_nulls(<col> || $N::JSONB)`.
	fn set_expr(&self, col: &str, param_idx: usize) -> String {
		match self {
			Self::Text(_) => format!("{col} = ${param_idx}::TEXT"),
			Self::Int(_) => format!("{col} = ${param_idx}::INTEGER"),
			Self::Bool(_) => format!("{col} = ${param_idx}::BOOLEAN"),
			Self::Json(_) => format!("{col} = ${param_idx}::JSONB"),
			Self::JsonMerge(_) => format!("{col} = jsonb_strip_nulls({col} || ${param_idx}::JSONB)"),
		}
	}
}

impl ModelConfig {
	/// Ensure a system-level (owner-less) config row exists for a model.
	///
	/// Inserts a minimal row if none exists yet; does nothing if one is already
	/// present. Returns the current row either way.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn ensure_system_config(conn: &mut sqlx::PgConnection, model_id: &Uuid, stable_key: &str, name: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
            INSERT INTO model_configs (owner_id, model_id, stable_key, name)
            VALUES (NULL, $1, $2, $3)
            ON CONFLICT (model_id) WHERE owner_id IS NULL DO UPDATE
                SET stable_key = EXCLUDED.stable_key,
                    name       = EXCLUDED.name,
                    updated_at = NOW()
            RETURNING *
            "#,
		)
		.bind(model_id)
		.bind(stable_key)
		.bind(name)
		.fetch_one(conn)
		.await
	}

	/// Apply a partial update to the system-level config for a model.
	///
	/// Only the columns named in `fields` are written; every other column is
	/// left untouched.  Pass `ConfigValue::Text(None)` / `ConfigValue::Json(None)`
	/// / `ConfigValue::Int(None)` to explicitly set a nullable column to NULL.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn patch_system_config(conn: &mut sqlx::PgConnection, model_id: &Uuid, fields: &[(&str, ConfigValue<'_>)]) -> Result<Self, sqlx::Error> {
		let set_clause = fields
			.iter()
			.enumerate()
			.map(|(i, (col, val))| val.set_expr(col, i + 2))
			.collect::<Vec<_>>()
			.join(", ");

		let sql = format!(
			"UPDATE model_configs SET {set_clause}, updated_at = NOW() \
             WHERE model_id = $1 AND owner_id IS NULL RETURNING *"
		);

		let mut q = sqlx::query_as::<_, ModelConfig>(&sql).bind(model_id);
		for (_, val) in fields {
			q = match val {
				ConfigValue::Text(v) => q.bind(v),
				ConfigValue::Int(v) => q.bind(v),
				ConfigValue::Bool(v) => q.bind(v),
				ConfigValue::Json(v) => q.bind(v.map(|j| sqlx::types::Json(j))),
				ConfigValue::JsonMerge(v) => q.bind(sqlx::types::Json(v)),
			};
		}
		q.fetch_one(conn).await
	}
}
