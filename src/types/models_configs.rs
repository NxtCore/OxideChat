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

impl ModelConfig {
	/// Insert or update the system-level (owner-less) config for a model.
	///
	/// Matches an existing system config by `model_id` where `owner_id IS NULL`.
	/// If one exists it is updated in-place; otherwise a new row is inserted.
	/// All optional fields are only applied when `Some`.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	#[allow(clippy::too_many_arguments)]
	pub async fn upsert_system_config(
		conn: &mut sqlx::PgConnection,
		model_id: &Uuid,
		stable_key: &str,
		name: &str,
		description: Option<&str>,
		icon: Option<&str>,
		system_prompt: Option<&str>,
		sampling: Option<&Value>,
		input_modalities: Option<&Value>,
		output_modalities: Option<&Value>,
		context_length: Option<i32>,
		max_output_tokens: Option<i32>,
		enabled_tools: Option<&Value>,
		is_public: Option<bool>,
		is_featured: Option<bool>,
		is_default: Option<bool>,
		is_favorite: Option<bool>,
		category: Option<&str>,
		tags: Option<&Value>,
		extra_settings: Option<&Value>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
            INSERT INTO model_configs (
                owner_id, model_id, stable_key, name,
                description, icon, system_prompt, sampling,
                input_modalities, output_modalities, context_length, max_output_tokens,
                enabled_tools, is_public, is_featured, is_default,
                is_favorite, category, tags, extra_settings
            )
            VALUES (
                NULL, $1, $2, $3,
                $4, $5, $6, COALESCE($7, '{}'),
                $8, $9, $10, $11,
                COALESCE($12, '[]'), COALESCE($13, false), COALESCE($14, false), COALESCE($15, false),
                COALESCE($16, false), $17, COALESCE($18, '[]'), COALESCE($19, '{}')
            )
            ON CONFLICT (model_id) WHERE owner_id IS NULL
            DO UPDATE SET
                stable_key        = EXCLUDED.stable_key,
                name              = EXCLUDED.name,
                description       = COALESCE(EXCLUDED.description,       model_configs.description),
                icon              = COALESCE(EXCLUDED.icon,              model_configs.icon),
                system_prompt     = COALESCE(EXCLUDED.system_prompt,     model_configs.system_prompt),
                sampling          = COALESCE(EXCLUDED.sampling,          model_configs.sampling),
                input_modalities  = COALESCE(EXCLUDED.input_modalities,  model_configs.input_modalities),
                output_modalities = COALESCE(EXCLUDED.output_modalities, model_configs.output_modalities),
                context_length    = COALESCE(EXCLUDED.context_length,    model_configs.context_length),
                max_output_tokens = COALESCE(EXCLUDED.max_output_tokens, model_configs.max_output_tokens),
                enabled_tools     = COALESCE(EXCLUDED.enabled_tools,     model_configs.enabled_tools),
                is_public         = COALESCE(EXCLUDED.is_public,         model_configs.is_public),
                is_featured       = COALESCE(EXCLUDED.is_featured,       model_configs.is_featured),
                is_default        = COALESCE(EXCLUDED.is_default,        model_configs.is_default),
                is_favorite       = COALESCE(EXCLUDED.is_favorite,       model_configs.is_favorite),
                category          = COALESCE(EXCLUDED.category,          model_configs.category),
                tags              = COALESCE(EXCLUDED.tags,              model_configs.tags),
                extra_settings    = COALESCE(EXCLUDED.extra_settings,    model_configs.extra_settings),
                updated_at        = NOW()
            RETURNING *
            "#,
		)
		.bind(model_id)
		.bind(stable_key)
		.bind(name)
		.bind(description)
		.bind(icon)
		.bind(system_prompt)
		.bind(sampling)
		.bind(input_modalities)
		.bind(output_modalities)
		.bind(context_length)
		.bind(max_output_tokens)
		.bind(enabled_tools)
		.bind(is_public)
		.bind(is_featured)
		.bind(is_default)
		.bind(is_favorite)
		.bind(category)
		.bind(tags)
		.bind(extra_settings)
		.fetch_one(conn)
		.await
	}
}
