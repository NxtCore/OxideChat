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
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
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
	const TABLE: &'static str = "models";
	const ALIAS: &'static str = "m";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			provider_id: Uuid::new_v4(),
			model_id: String::new(),
			display_name: String::new(),
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

	fn sql_fields() -> &'static [&'static str] {
		&[
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

enum ConfigValue<'a> {
	Text(Option<&'a str>),
	Bool(bool),
}

impl ConfigValue<'_> {
	fn set_expr(&self, col: &str, param_idx: usize) -> String {
		match self {
			Self::Text(_) => format!("{col} = ${param_idx}::TEXT"),
			Self::Bool(_) => format!("{col} = ${param_idx}::BOOLEAN"),
		}
	}
}

pub enum ModelPatchField<'a> {
	DisplayName(&'a str),
	IsEnabled(bool),
}

impl<'a> ModelPatchField<'a> {
	fn column_and_value(&self) -> (&'static str, ConfigValue<'a>) {
		match self {
			Self::DisplayName(v) => ("display_name", ConfigValue::Text(Some(v))),
			Self::IsEnabled(v) => ("is_enabled", ConfigValue::Bool(*v)),
		}
	}
}

impl Model {
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
	}

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
	pub async fn list_paginated(pool: &sqlx::PgPool, page: i64, size: i64, show_disabled: bool) -> Result<PaginatedResponse<ModelListPublic>, sqlx::Error> {
		let offset = (page - 1) * size;
		let limit = if size <= 0 { None } else { Some(size + 1) };

		let where_clause = if show_disabled {
			String::new()
		} else {
			format!("WHERE {}.is_enabled = true AND {}.is_enabled = true", Model::ALIAS, Provider::ALIAS)
		};

		let model_fields = Model::aliased_fields_str_from_list(&[
			"id",
			"model_id",
			"display_name",
			"capabilities",
			"input_modalities",
			"output_modalities",
			"context_length",
			"max_tokens",
			"is_enabled",
		]);

		let provider_fields = Provider::aliased_fields_str_from_list(&["name", "kind", "id"]);
		let model_config_fields = ModelConfig::aliased_fields_str_from_list(&["icon", "is_favorite"]);

		let query = format!(
			r#"
            SELECT {model_fields}, {provider_fields}, {model_config_fields}
            FROM {model_table} {model_alias}
            JOIN {provider_table} {provider_alias} ON {model_alias}.provider_id = {provider_alias}.id
            LEFT JOIN {model_config_table} {model_config_alias}
                ON {model_config_alias}.model_id = {model_alias}.id
                AND {model_config_alias}.owner_id IS NULL
            {where_clause}
            ORDER BY {model_alias}.display_name, {provider_alias}.name
            LIMIT $1 OFFSET $2
            "#,
			model_alias = Model::ALIAS,
			model_table = Model::TABLE,
			model_config_alias = ModelConfig::ALIAS,
			model_config_table = ModelConfig::TABLE,
			provider_alias = Provider::ALIAS,
			provider_table = Provider::TABLE,
		);

		let rows = sqlx::query(&query).bind(limit).bind(offset).fetch_all(pool).await?;

		let has_more = limit.is_some() && rows.len() > size as usize;
		let items = rows
			.into_iter()
			.take(if size <= 0 { usize::MAX } else { size as usize })
			.map(|row| {
				let provider_name: String = row.get("p_name");
				ModelListPublic {
					id: row.get("m_id"),
					model_id: row.get("m_model_id"),
					display_name: row.get("m_display_name"),
					capabilities: row.get::<Json<Vec<String>>, _>("m_capabilities").0,
					input_modalities: row.get::<Json<Vec<String>>, _>("m_input_modalities").0,
					output_modalities: row.get::<Json<Vec<String>>, _>("m_output_modalities").0,
					context_length: row.get("m_context_length"),
					max_tokens: row.get("m_max_tokens"),
					is_enabled: row.get("m_is_enabled"),
					provider: ProviderSlim {
						id: row.get("p_id"),
						name: provider_name.clone(),
						kind: row.get("p_kind"),
					},
					icon: row.get("mc_icon"),
					is_favorite: row.get::<Option<bool>, _>("mc_is_favorite").unwrap_or(false),
				}
			})
			.collect();

		Ok(PaginatedResponse { has_more, items })
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

		let model_fields = Model::aliased_fields_str_from_list(&[
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
		]);

		let provider_fields = Provider::aliased_fields_str_from_list(&["name", "kind", "id"]);
		let model_config_fields = ModelConfig::aliased_fields_str_from_list(&["icon"]);

		let query = format!(
			r#"
            SELECT {model_fields}, {provider_fields}, {model_config_fields}
            FROM {model_table} {model_alias}
            JOIN {provider_table} {provider_alias} ON {model_alias}.provider_id = {provider_alias}.id
            LEFT JOIN {model_config_table} {model_config_alias}
                ON {model_config_alias}.model_id = {model_alias}.id
                AND {model_config_alias}.owner_id IS NULL
            WHERE {model_alias}.display_name ILIKE $3 ESCAPE '\' OR {provider_alias}.name ILIKE $3 ESCAPE '\'
            ORDER BY {model_alias}.display_name, {provider_alias}.name
            LIMIT $1 OFFSET $2
            "#,
			model_alias = Model::ALIAS,
			model_table = Model::TABLE,
			model_config_alias = ModelConfig::ALIAS,
			model_config_table = ModelConfig::TABLE,
			provider_alias = Provider::ALIAS,
			provider_table = Provider::TABLE,
		);

		let rows = sqlx::query(&query)
			.bind(limit)
			.bind(offset)
			.bind(search_query.as_deref().map(|s| format!("%{}%", Self::escape_like_pattern(s))).unwrap_or_else(|| "%".to_string()))
			.fetch_all(pool)
			.await?;

		let has_more = limit.is_some() && rows.len() > size as usize;
		let items = rows
			.into_iter()
			.take(if size <= 0 { usize::MAX } else { size as usize })
			.map(|row| ModelListAdmin {
				id: row.get("m_id"),
				model_id: row.get("m_model_id"),
				display_name: row.get("m_display_name"),
				capabilities: row.get::<Json<Vec<String>>, _>("m_capabilities").0,
				input_modalities: row.get::<Json<Vec<String>>, _>("m_input_modalities").0,
				output_modalities: row.get::<Json<Vec<String>>, _>("m_output_modalities").0,
				context_length: row.get("m_context_length"),
				max_tokens: row.get("m_max_tokens"),
				is_enabled: row.get("m_is_enabled"),
				created_at: row.get("m_created_at"),
				updated_at: row.get("m_updated_at"),
				provider: ProviderSlim {
					id: row.get("p_id"),
					name: row.get("p_name"),
					kind: row.get("p_kind"),
				},
				icon: row.get("mc_icon"),
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
		let model_fields = Model::aliased_fields_str_from_list(&[
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
		]);

		let provider_fields = Provider::aliased_fields_str_from_list(&["name", "kind", "id"]);
		let model_config_fields = ModelConfig::aliased_fields_str_from_list(&[
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
			"tags",
		]);

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
			model_alias = Model::ALIAS,
			model_table = Model::TABLE,
			model_config_alias = ModelConfig::ALIAS,
			model_config_table = ModelConfig::TABLE,
			provider_alias = Provider::ALIAS,
			provider_table = Provider::TABLE,
		);

		let row = match sqlx::query(&query).bind(id).fetch_optional(pool).await? {
			Some(r) => r,
			None => return Ok(None),
		};

		Ok(Some(ModelDetailed {
			id: row.get("m_id"),
			model_id: row.get("m_model_id"),
			display_name: row.get("m_display_name"),
			capabilities: row.get::<Json<Vec<String>>, _>("m_capabilities").0,
			input_modalities: row.get::<Json<Vec<String>>, _>("m_input_modalities").0,
			output_modalities: row.get::<Json<Vec<String>>, _>("m_output_modalities").0,
			context_length: row.get("m_context_length"),
			max_tokens: row.get("m_max_tokens"),
			is_enabled: row.get("m_is_enabled"),
			created_at: row.get("m_created_at"),
			updated_at: row.get("m_updated_at"),
			provider: ProviderSlim {
				id: row.get("p_id"),
				name: row.get("p_name"),
				kind: row.get("p_kind"),
			},
			icon: row.get("mc_icon"),
			description: row.get("mc_description"),
			system_prompt: row.get("mc_system_prompt"),
			sampling: row.get::<Option<Json<Value>>, _>("mc_sampling").map(|j| j.0),
			extra_settings: row
				.get::<Option<Json<Value>>, _>("mc_extra_settings")
				.map(|j| j.0),
			is_public: row.get::<Option<bool>, _>("mc_is_public").unwrap_or(false),
			is_featured: row.get::<Option<bool>, _>("mc_is_featured").unwrap_or(false),
			is_default: row.get::<Option<bool>, _>("mc_is_default").unwrap_or(false),
			is_favorite: row.get::<Option<bool>, _>("mc_is_favorite").unwrap_or(false),
			category: row.get("mc_category"),
			tags: row
				.get::<Option<Json<Vec<String>>>, _>("mc_tags")
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

	/// Apply a partial update to a model row.
	///
	/// Each field in `fields` sets exactly one column; columns not listed are
	/// left untouched. `updated_at` is bumped automatically.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn patch_via_connection(conn: &mut sqlx::PgConnection, id: &Uuid, fields: &[ModelPatchField<'_>]) -> Result<Option<Model>, sqlx::Error> {
		if fields.is_empty() {
			return sqlx::query_as::<_, Model>("SELECT * FROM models WHERE id = $1")
				.bind(id)
				.fetch_optional(&mut *conn)
				.await;
		}

		let set_clause = fields
			.iter()
			.enumerate()
			.map(|(i, field)| {
				let (col, val) = field.column_and_value();
				val.set_expr(col, i + 2)
			})
			.collect::<Vec<_>>()
			.join(", ");

		let sql = format!(
			"UPDATE models SET {set_clause}, updated_at = NOW() WHERE id = $1 RETURNING *"
		);

		let mut q = sqlx::query_as::<_, Model>(&sql).bind(id);
		for field in fields {
			let (_, val) = field.column_and_value();
			q = match val {
				ConfigValue::Text(v) => q.bind(v),
				ConfigValue::Bool(v) => q.bind(v),
			};
		}
		q.fetch_optional(&mut *conn).await
	}

	/// Find display_name and model_id by ID.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn find_name_and_model_id<'e, E>(executor: E, id: &Uuid) -> Result<Option<(String, String)>, sqlx::Error>
	where
		E: sqlx::Executor<'e, Database = sqlx::Postgres>,
	{
		sqlx::query_as::<_, (String, String)>(
			r#"
            SELECT display_name, model_id
            FROM models
            WHERE id = $1
            "#,
		)
		.bind(id)
		.fetch_optional(executor)
		.await
	}
}
