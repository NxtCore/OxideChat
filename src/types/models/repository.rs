use super::rows::{ModelDetailedRow, ModelListAdminRow, ModelListPublicRow};
use super::{Model, ModelDetailed, ModelListAdmin, ModelListPublic, ModelViewer};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use crate::types::providers::ProviderKind;
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

impl Model {
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
	}

	pub async fn create(pool: &sqlx::PgPool, provider_id: &Uuid, model_id: &str, display_name: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as!(
			Model,
			r#"
			INSERT INTO models (provider_id, model_id, display_name, capabilities, input_modalities, output_modalities)
			VALUES ($1, $2, $3, '[]'::JSONB, '["text"]'::JSONB, '["text"]'::JSONB)
			RETURNING
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS "is_enabled!",
				created_at,
				updated_at
			"#,
			provider_id,
			model_id,
			display_name,
		)
		.fetch_one(pool)
		.await
	}

	pub async fn list_for_user(
		pool: &sqlx::PgPool,
		viewer: ModelViewer<'_>,
		page: i64,
		size: i64,
		show_disabled: bool,
	) -> Result<PaginatedResponse<ModelListPublic>, sqlx::Error> {
		let pagination = Self::pagination(page, size);

		let rows = sqlx::query_as!(
			ModelListPublicRow,
			r#"
			SELECT
				m.id,
				m.model_id,
				m.display_name,
				COALESCE(user_mc.capabilities, sys_mc.capabilities, m.capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(user_mc.input_modalities, sys_mc.input_modalities, m.input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(user_mc.output_modalities, sys_mc.output_modalities, m.output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				COALESCE(user_mc.context_length, sys_mc.context_length, m.context_length) AS context_length,
				COALESCE(user_mc.max_output_tokens, sys_mc.max_output_tokens, m.max_tokens) AS max_tokens,
				COALESCE(m.is_enabled, false) AS "is_enabled!",
				p.id AS provider_id,
				p.name AS provider_name,
				p.kind AS "provider_kind: ProviderKind",
				COALESCE(user_mc.icon, sys_mc.icon) AS icon,
				COALESCE(user_mc.is_favorite, false) AS "is_favorite!"
			FROM models m
			JOIN providers p ON m.provider_id = p.id
			LEFT JOIN model_configs sys_mc
				ON sys_mc.model_id = m.id
				AND sys_mc.owner_id IS NULL
			LEFT JOIN model_configs user_mc
				ON user_mc.model_id = m.id
				AND user_mc.owner_id = $1
			WHERE (
				$4::BOOLEAN = TRUE
				OR (
					COALESCE(m.is_enabled, false) = TRUE
					AND COALESCE(p.is_enabled, false) = TRUE
				)
			)
			ORDER BY m.display_name, p.name
			LIMIT $2 OFFSET $3
			"#,
			viewer.user_id,
			pagination.limit,
			pagination.offset,
			show_disabled,
		)
		.fetch_all(pool)
		.await?;

		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(ModelListPublic::from).collect();

		Ok(PaginatedResponse { has_more, items })
	}

	pub async fn list_for_admin(pool: &sqlx::PgPool, page: i64, size: i64, search_query: Option<String>) -> Result<PaginatedResponse<ModelListAdmin>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let search = search_query
			.as_deref()
			.map(str::trim)
			.filter(|s| !s.is_empty())
			.map(|s| format!("%{}%", Self::escape_like_pattern(s)));

		let rows = sqlx::query_as!(
			ModelListAdminRow,
			r#"
			SELECT
				m.id,
				m.model_id,
				m.display_name,
				COALESCE(sys_mc.capabilities, m.capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(sys_mc.input_modalities, m.input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(sys_mc.output_modalities, m.output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				COALESCE(sys_mc.context_length, m.context_length) AS context_length,
				COALESCE(sys_mc.max_output_tokens, m.max_tokens) AS max_tokens,
				COALESCE(m.is_enabled, false) AS "is_enabled!",
				m.created_at,
				m.updated_at,
				p.id AS provider_id,
				p.name AS provider_name,
				p.kind AS "provider_kind: ProviderKind",
				sys_mc.icon
			FROM models m
			JOIN providers p ON m.provider_id = p.id
			LEFT JOIN model_configs sys_mc
				ON sys_mc.model_id = m.id
				AND sys_mc.owner_id IS NULL
			WHERE (
				$3::TEXT IS NULL
				OR m.display_name ILIKE $3 ESCAPE '\'
				OR p.name ILIKE $3 ESCAPE '\'
			)
			ORDER BY m.display_name, p.name
			LIMIT $1 OFFSET $2
			"#,
			pagination.limit,
			pagination.offset,
			search.as_deref(),
		)
		.fetch_all(pool)
		.await?;

		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(ModelListAdmin::from).collect();

		Ok(PaginatedResponse { has_more, items })
	}

	pub async fn find_for_admin(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<ModelDetailed>, sqlx::Error> {
		let row = sqlx::query_as!(
			ModelDetailedRow,
			r#"
			SELECT
				m.id,
				m.created_at,
				m.updated_at,
				m.model_id,
				m.display_name,
				COALESCE(sys_mc.capabilities, m.capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(sys_mc.input_modalities, m.input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(sys_mc.output_modalities, m.output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				COALESCE(sys_mc.context_length, m.context_length) AS context_length,
				COALESCE(sys_mc.max_output_tokens, m.max_tokens) AS max_tokens,
				COALESCE(m.is_enabled, false) AS "is_enabled!",
				p.id AS provider_id,
				p.name AS provider_name,
				p.kind AS "provider_kind: ProviderKind",
				sys_mc.icon,
				sys_mc.description,
				sys_mc.system_prompt,
				sys_mc.sampling AS "sampling: Json<Value>",
				sys_mc.extra_settings AS "extra_settings: Json<Value>",
				COALESCE(sys_mc.is_public, false) AS "is_public!",
				COALESCE(sys_mc.is_featured, false) AS "is_featured!",
				COALESCE(sys_mc.is_default, false) AS "is_default!",
				COALESCE(sys_mc.is_favorite, false) AS "is_favorite!",
				sys_mc.category,
				COALESCE(sys_mc.tags, '[]'::jsonb) AS "tags!: Json<Vec<String>>"
			FROM models m
			JOIN providers p ON m.provider_id = p.id
			LEFT JOIN model_configs sys_mc
				ON sys_mc.model_id = m.id
				AND sys_mc.owner_id IS NULL
			WHERE m.id = $1
			"#,
			id,
		)
		.fetch_optional(pool)
		.await?;

		Ok(row.map(ModelDetailed::from))
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Model>, sqlx::Error> {
		sqlx::query_as!(
			Model,
			r#"
			SELECT
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS "is_enabled!",
				created_at,
				updated_at
			FROM models
			WHERE id = $1
			"#,
			id,
		)
		.fetch_optional(pool)
		.await
	}

	pub async fn find_name_and_model_id<'e, E>(executor: E, id: &Uuid) -> Result<Option<(String, String)>, sqlx::Error>
	where
		E: sqlx::Executor<'e, Database = sqlx::Postgres>,
	{
		let row = sqlx::query!(
			r#"
			SELECT display_name, model_id
			FROM models
			WHERE id = $1
			"#,
			id,
		)
		.fetch_optional(executor)
		.await?;

		Ok(row.map(|r| (r.display_name, r.model_id)))
	}
}
