use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use crate::types::models_configs::ModelConfig;
use crate::types::providers::{Provider, ProviderSlim};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Model {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Json<Vec<String>>,
	pub input_modalities: Json<Vec<String>>,
	pub output_modalities: Json<Vec<String>>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ModelListPublic {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider: ProviderSlim,
	pub provider_name: String,
	pub icon: Option<String>,
	pub is_favorite: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelDetailed {
	pub id: Uuid,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider: ProviderSlim,
	pub icon: Option<String>,
	pub description: Option<String>,
	pub system_prompt: Option<String>,
	pub sampling: Option<Value>,
	pub extra_settings: Option<Value>,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub is_favorite: bool,
	pub category: Option<String>,
	pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelListAdmin {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub provider: ProviderSlim,
	pub icon: Option<String>,
}

impl BaseType for Model {
	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			provider_id: Uuid::new_v4(),
			model_id: "".to_string(),
			display_name: "".to_string(),
			capabilities: Json(vec![]),
			input_modalities: Json(vec![]),
			output_modalities: Json(vec![]),
			context_length: None,
			max_tokens: None,
			is_enabled: true,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}
	fn table(&self) -> &str {
		"models"
	}
	fn alias(&self) -> &str {
		"m"
	}
	fn sql_fields(&self) -> Vec<&str> {
		vec![
			"id",
			"provider_id",
			"model_id",
			"display_name",
			"capabilities",
			"input_modalities",
			"output_modalities",
			"context_length",
			"max_tokens",
			"is_enabled",
			"created_at",
			"updated_at",
		]
	}
}

impl Model {
	/// Create a new model linked to a provider.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn create(pool: &sqlx::PgPool, provider_id: &Uuid, model_id: &str, display_name: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Model>(
			r#"
            INSERT INTO models (provider_id, model_id, display_name)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
		)
		.bind(provider_id)
		.bind(model_id)
		.bind(display_name)
		.fetch_one(pool)
		.await
	}

	/// List models for regular users with pagination.
	///
	/// Joins the system-level config (`owner_id IS NULL`) for each model to surface
	/// `is_favorite` and `icon`. Pass `size <= 0` to return all
	/// models without a limit.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn list_paginated(pool: &sqlx::PgPool, page: i64, size: i64) -> Result<PaginatedResponse<ModelListPublic>, sqlx::Error> {
		let offset = (page - 1) * size;
		let limit = if size <= 0 { None } else { Some(size + 1) };

		let model = Model::new();
		let model_config = ModelConfig::new();
		let provider = Provider::new();
		let query = format!(
			r#"
            SELECT {model_fields}, {provider_fields}, {model_config_fields}
            FROM {model_table} {model_alias}
            JOIN {provider_table} {provider_alias} ON {model_alias}.provider_id = {provider_alias}.id
            LEFT JOIN {model_config_table} {model_config_alias}
                ON {model_config_alias}.model_id = {model_alias}.id
                AND {model_config_alias}.owner_id IS NULL
            ORDER BY {model_alias}.display_name, {provider_alias}.name
            LIMIT $1 OFFSET $2
            "#,
			model_fields = model.aliased_fields_str_from_list(vec![
				"id",
				"model_id",
				"display_name",
				"capabilities",
				"input_modalities",
				"output_modalities",
				"context_length",
				"max_tokens",
				"is_enabled"
			]),
			model_alias = model.alias(),
			model_table = model.table(),
			model_config_fields = model_config.aliased_fields_str_from_list(vec!["icon", "is_favorite"]),
			model_config_alias = model_config.alias(),
			model_config_table = model_config.table(),
			provider_fields = provider.aliased_fields_str_from_list(vec!["name", "kind", "id"]),
			provider_alias = provider.alias(),
			provider_table = provider.table(),
		);

		let rows = sqlx::query(&query).bind(limit).bind(offset).fetch_all(pool).await?;
		let items = rows
			.into_iter()
			.take(if size <= 0 { usize::MAX } else { size as usize })
			.map(|row| {
				let provider_name: String = row.get(format!("{}_name", provider.alias()).as_str());
				ModelListPublic {
					id: row.get(format!("{}_id", model.alias()).as_str()),
					model_id: row.get(format!("{}_model_id", model.alias()).as_str()),
					display_name: row.get(format!("{}_display_name", model.alias()).as_str()),
					capabilities: row.get::<Json<Vec<String>>, _>(format!("{}_capabilities", model.alias()).as_str()).0,
					input_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_input_modalities", model.alias()).as_str()).0,
					output_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_output_modalities", model.alias()).as_str()).0,
					context_length: row.get::<Option<i32>, _>(format!("{}_context_length", model.alias()).as_str()),
					max_tokens: row.get::<Option<i32>, _>(format!("{}_max_tokens", model.alias()).as_str()),
					is_enabled: row.get(format!("{}_is_enabled", model.alias()).as_str()),
					provider: ProviderSlim {
						id: row.get(format!("{}_id", provider.alias()).as_str()),
						name: provider_name.clone(),
						kind: row.get(format!("{}_kind", provider.alias()).as_str()),
					},
					provider_name,
					icon: row.get(format!("{}_icon", model_config.alias()).as_str()),
					is_favorite: row.get::<Option<bool>, _>(format!("{}_is_favorite", model_config.alias()).as_str()).unwrap_or(false),
				}
			})
			.collect();
		Ok(PaginatedResponse { has_more: false, items })
	}

	/// List all models for admin with pagination, including system config fields.
	///
	/// Joins the system-level config (`owner_id IS NULL`) and the provider so the
	/// admin listing has full context. Pass `size <= 0` to return all models.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn list_paginated_admin(pool: &sqlx::PgPool, page: i64, size: i64, search_query: Option<String>) -> Result<PaginatedResponse<ModelListAdmin>, sqlx::Error> {
		let offset = (page - 1) * size;
		let limit = if size <= 0 { None } else { Some(size + 1) };

		let model = Model::new();
		let model_config = ModelConfig::new();
		let provider = Provider::new();
		let query = format!(
			r#"
            SELECT {model_fields}, {provider_fields}, {model_config_fields}
            FROM {model_table} {model_alias}
            JOIN {provider_table} {provider_alias} ON {model_alias}.provider_id = {provider_alias}.id
            LEFT JOIN {model_config_table} {model_config_alias}
                ON {model_config_alias}.model_id = {model_alias}.id
                AND {model_config_alias}.owner_id IS NULL
            WHERE {model_alias}.display_name ILIKE $3 OR {provider_alias}.name ILIKE $3
            ORDER BY {model_alias}.display_name, {provider_alias}.name
            LIMIT $1 OFFSET $2
            "#,
			model_fields = model.aliased_fields_str_from_list(vec![
				"id",
				"provider_id",
				"model_id",
				"display_name",
				"capabilities",
				"input_modalities",
				"output_modalities",
				"context_length",
				"max_tokens",
				"is_enabled",
				"created_at",
				"updated_at"
			]),
			model_alias = model.alias(),
			model_table = model.table(),
			model_config_fields = model_config.aliased_fields_str_from_list(vec!["icon",]),
			model_config_alias = model_config.alias(),
			model_config_table = model_config.table(),
			provider_fields = provider.aliased_fields_str_from_list(vec!["name", "kind", "id"]),
			provider_alias = provider.alias(),
			provider_table = provider.table(),
		);

		let rows = sqlx::query(&query)
			.bind(limit)
			.bind(offset)
			.bind(search_query.as_deref().map(|s| format!("%{}%", s)).unwrap_or("%".to_string()))
			.fetch_all(pool)
			.await?;

		let has_more = rows.len() > size as usize;
		let items = rows
			.into_iter()
			.take(if size <= 0 { usize::MAX } else { size as usize })
			.map(|row| ModelListAdmin {
				id: row.get(format!("{}_id", model.alias()).as_str()),
				model_id: row.get(format!("{}_model_id", model.alias()).as_str()),
				display_name: row.get(format!("{}_display_name", model.alias()).as_str()),
				capabilities: row.get::<Json<Vec<String>>, _>(format!("{}_capabilities", model.alias()).as_str()).0,
				input_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_input_modalities", model.alias()).as_str()).0,
				output_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_output_modalities", model.alias()).as_str()).0,
				context_length: row.get::<Option<i32>, _>(format!("{}_context_length", model.alias()).as_str()),
				max_tokens: row.get::<Option<i32>, _>(format!("{}_max_tokens", model.alias()).as_str()),
				is_enabled: row.get(format!("{}_is_enabled", model.alias()).as_str()),
				created_at: row.get(format!("{}_created_at", model.alias()).as_str()),
				updated_at: row.get(format!("{}_updated_at", model.alias()).as_str()),
				provider: ProviderSlim {
					id: row.get(format!("{}_id", provider.alias()).as_str()),
					name: row.get(format!("{}_name", provider.alias()).as_str()),
					kind: row.get(format!("{}_kind", provider.alias()).as_str()),
				},
				icon: row.get(format!("{}_icon", model_config.alias()).as_str()),
			})
			.collect();

		Ok(PaginatedResponse { has_more, items })
	}

	/// Find a single model by ID, joined with its provider and system config.
	///
	/// Used by the admin detail endpoint to return the full `ModelDetailed`.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn find_by_id_with_config(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<ModelDetailed>, sqlx::Error> {
		let model = Model::new();
		let model_config = ModelConfig::new();
		let provider = Provider::new();
		let query = format!(
			r#"
            SELECT {model_fields}, {provider_fields}, {model_config_fields}
            FROM {model_table} {model_alias}
            JOIN {provider_table} {provider_alias} ON {model_alias}.provider_id = {provider_alias}.id
            LEFT JOIN {model_config_table} {model_config_alias}
                ON {model_config_alias}.model_id = {model_alias}.id
                AND {model_config_alias}.owner_id IS NULL
            WHERE {model_alias}.id = $1
            "#,
			model_fields = model.aliased_fields_str_from_list(vec![
				"id",
				"provider_id",
				"model_id",
				"display_name",
				"capabilities",
				"input_modalities",
				"output_modalities",
				"context_length",
				"max_tokens",
				"is_enabled",
				"created_at",
				"updated_at"
			]),
			model_alias = model.alias(),
			model_table = model.table(),
			model_config_fields = model_config.aliased_fields_str_from_list(vec![
				"icon",
				"description",
				"system_prompt",
				"sampling",
				"extra_settings",
				"is_public",
				"is_featured",
				"is_default",
				"is_favorite",
				"category",
				"tags"
			]),
			model_config_alias = model_config.alias(),
			model_config_table = model_config.table(),
			provider_fields = provider.aliased_fields_str_from_list(vec!["name", "kind", "id"]),
			provider_alias = provider.alias(),
			provider_table = provider.table(),
		);

		let row = match sqlx::query(&query).bind(id).fetch_optional(pool).await? {
			Some(r) => r,
			None => return Ok(None),
		};

		Ok(Some(ModelDetailed {
			id: row.get(format!("{}_id", model.alias()).as_str()),
			model_id: row.get(format!("{}_model_id", model.alias()).as_str()),
			display_name: row.get(format!("{}_display_name", model.alias()).as_str()),
			capabilities: row.get::<Json<Vec<String>>, _>(format!("{}_capabilities", model.alias()).as_str()).0,
			input_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_input_modalities", model.alias()).as_str()).0,
			output_modalities: row.get::<Json<Vec<String>>, _>(format!("{}_output_modalities", model.alias()).as_str()).0,
			context_length: row.get::<Option<i32>, _>(format!("{}_context_length", model.alias()).as_str()),
			max_tokens: row.get::<Option<i32>, _>(format!("{}_max_tokens", model.alias()).as_str()),
			is_enabled: row.get(format!("{}_is_enabled", model.alias()).as_str()),
			created_at: row.get(format!("{}_created_at", model.alias()).as_str()),
			updated_at: row.get(format!("{}_updated_at", model.alias()).as_str()),
			provider: ProviderSlim {
				id: row.get(format!("{}_id", provider.alias()).as_str()),
				name: row.get(format!("{}_name", provider.alias()).as_str()),
				kind: row.get(format!("{}_kind", provider.alias()).as_str()),
			},
			icon: row.get(format!("{}_icon", model_config.alias()).as_str()),
			description: row.get(format!("{}_description", model_config.alias()).as_str()),
			system_prompt: row.get(format!("{}_system_prompt", model_config.alias()).as_str()),
			sampling: row.get::<Option<Json<Value>>, _>(format!("{}_sampling", model_config.alias()).as_str()).map(|j| j.0),
			extra_settings: row
				.get::<Option<Json<Value>>, _>(format!("{}_extra_settings", model_config.alias()).as_str())
				.map(|j| j.0),
			is_public: row.get::<Option<bool>, _>(format!("{}_is_public", model_config.alias()).as_str()).unwrap_or(false),
			is_featured: row.get::<Option<bool>, _>(format!("{}_is_featured", model_config.alias()).as_str()).unwrap_or(false),
			is_default: row.get::<Option<bool>, _>(format!("{}_is_default", model_config.alias()).as_str()).unwrap_or(false),
			is_favorite: row.get::<Option<bool>, _>(format!("{}_is_favorite", model_config.alias()).as_str()).unwrap_or(false),
			category: row.get(format!("{}_category", model_config.alias()).as_str()),
			tags: row
				.get::<Option<Json<Vec<String>>>, _>(format!("{}_tags", model_config.alias()).as_str())
				.map(|j| j.0)
				.unwrap_or_default(),
		}))
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Model>, sqlx::Error> {
		sqlx::query_as::<_, Model>(
			r#"
            SELECT *
            FROM models
            WHERE id = $1
            "#,
		)
		.bind(id)
		.fetch_optional(pool)
		.await
	}

	pub async fn update(pool: &sqlx::PgPool, id: &Uuid, fields: &[(&str, &str)]) -> Result<Option<Model>, sqlx::Error> {
		let set_clause = fields
			.iter()
			.enumerate()
			.map(|(i, (field, _))| format!("{} = ${}", field, i + 2))
			.collect::<Vec<_>>()
			.join(", ");

		let query = format!("UPDATE models SET {}, updated_at = NOW() WHERE id = $1 RETURNING *", set_clause);

		let mut q = sqlx::query_as::<_, Model>(&query).bind(id);
		for (_, value) in fields {
			q = q.bind(value);
		}
		q.fetch_optional(pool).await
	}

	pub async fn update_via_connection(conn: &mut sqlx::PgConnection, id: &Uuid, fields: &[(&str, &Value)]) -> Result<Option<Model>, sqlx::Error> {
		let set_clause = fields
			.iter()
			.enumerate()
			.map(|(i, (field, _))| format!("{} = ${}", field, i + 2))
			.collect::<Vec<_>>()
			.join(", ");

		let query = format!("UPDATE models SET {}, updated_at = NOW() WHERE id = $1 RETURNING *", set_clause);

		let mut q = sqlx::query_as::<_, Model>(&query).bind(id);
		for (_, value) in fields {
			q = q.bind(value);
		}
		q.fetch_optional(&mut *conn).await
	}
}
