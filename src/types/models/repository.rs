use super::rows::{ModelDetailedRow, ModelListAdminRow, ModelListPublicRow};
use super::{Model, ModelDetailed, ModelListAdmin, ModelListPublic, ModelSyncInput, ModelSyncSummary, ModelViewer};
use crate::types::BaseType;
use crate::types::PolicyResolver;
use crate::types::axum::PaginatedResponse;
use crate::types::models::ProviderTab;
use crate::types::providers::{ProviderKind, ProviderModelResponse};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::{BTreeMap, HashMap, HashSet};
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

	pub async fn model_keys_by_ids(pool: &sqlx::PgPool, model_ids: &[Uuid]) -> Result<HashMap<Uuid, String>, sqlx::Error> {
		if model_ids.is_empty() {
			return Ok(HashMap::new());
		}

		let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, model_id FROM models WHERE id = ANY($1)")
			.bind(model_ids)
			.fetch_all(pool)
			.await?;

		Ok(rows.into_iter().collect())
	}

	pub async fn list_for_user(
		pool: &sqlx::PgPool,
		viewer: ModelViewer<'_>,
		page: i64,
		size: i64,
		show_disabled: bool,
		search_query: Option<&str>,
		favorites_only: bool,
		provider_id: Option<&Uuid>,
	) -> Result<PaginatedResponse<ModelListPublic>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let search = search_query
			.map(str::trim)
			.filter(|s| !s.is_empty())
			.map(|s| format!("%{}%", Self::escape_like_pattern(s)));

		let rows = sqlx::query_as::<_, ModelListPublicRow>(
			r#"
			SELECT
				m.id,
				m.model_id,
				m.display_name,
				COALESCE(user_mc.capabilities, sys_mc.capabilities, m.capabilities, '[]'::jsonb) AS capabilities,
				COALESCE(user_mc.input_modalities, sys_mc.input_modalities, m.input_modalities, '[]'::jsonb) AS input_modalities,
				COALESCE(user_mc.output_modalities, sys_mc.output_modalities, m.output_modalities, '[]'::jsonb) AS output_modalities,
				COALESCE(user_mc.context_length, sys_mc.context_length, m.context_length) AS context_length,
				COALESCE(user_mc.max_output_tokens, sys_mc.max_output_tokens, m.max_tokens) AS max_tokens,
				COALESCE(m.is_enabled, false) AS is_enabled,
				p.id AS provider_id,
				p.name AS provider_name,
				p.kind AS provider_kind,
				COALESCE(user_mc.icon, sys_mc.icon) AS icon,
				COALESCE(user_mc.is_favorite, false) AS is_favorite
			FROM models m
			JOIN providers p ON m.provider_id = p.id
			LEFT JOIN model_configs sys_mc
				ON sys_mc.model_id = m.id
				AND sys_mc.owner_id IS NULL
			LEFT JOIN model_configs user_mc
				ON user_mc.model_id = m.id
				AND user_mc.owner_id = $1
			WHERE (
				$4 = TRUE
				OR (
					COALESCE(m.is_enabled, false) = TRUE
					AND COALESCE(p.is_enabled, false) = TRUE
				)
			)
			AND (
				$5::TEXT IS NULL
				OR m.display_name ILIKE $5 ESCAPE '\'
				OR m.model_id ILIKE $5 ESCAPE '\'
			)
			AND (
				$6 = FALSE
				OR COALESCE(user_mc.is_favorite, false) = TRUE
			)
			AND (
				$7::UUID IS NULL
				OR p.id = $7
			)
			AND EXISTS (
				SELECT 1
				FROM team_members tm
				INNER JOIN teams t ON t.id = tm.team_id
				LEFT JOIN team_model_access tma_model ON tma_model.team_id = t.id AND tma_model.model_id = m.id
				LEFT JOIN team_model_access tma_provider ON tma_provider.team_id = t.id AND tma_provider.provider_id = p.id
				WHERE tm.user_id = $1
				  AND (t.allow_all_models = true OR tma_model.id IS NOT NULL OR tma_provider.id IS NOT NULL)
			)
			ORDER BY m.display_name, p.name
			LIMIT $2 OFFSET $3
			"#,
		)
		.bind(viewer.user_id)
		.bind(pagination.limit)
		.bind(pagination.offset)
		.bind(show_disabled)
		.bind(search.as_deref())
		.bind(favorites_only)
		.bind(provider_id)
		.fetch_all(pool)
		.await?;

		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(ModelListPublic::from).collect();

		Ok(PaginatedResponse { has_more, items })
	}

	pub async fn list_providers_for_user(pool: &sqlx::PgPool, viewer: ModelViewer<'_>) -> Result<Vec<ProviderTab>, sqlx::Error> {
		let rows = sqlx::query_as::<_, ProviderTab>(
			r#"
			SELECT DISTINCT p.id, p.name
			FROM providers p
			JOIN models m ON m.provider_id = p.id
			WHERE (
				COALESCE(m.is_enabled, false) = TRUE
				AND COALESCE(p.is_enabled, false) = TRUE
			)
			AND EXISTS (
				SELECT 1
				FROM team_members tm
				INNER JOIN teams t ON t.id = tm.team_id
				LEFT JOIN team_model_access tma_model ON tma_model.team_id = t.id AND tma_model.model_id = m.id
				LEFT JOIN team_model_access tma_provider ON tma_provider.team_id = t.id AND tma_provider.provider_id = p.id
				WHERE tm.user_id = $1
				  AND (t.allow_all_models = true OR tma_model.id IS NOT NULL OR tma_provider.id IS NOT NULL)
			)
			ORDER BY p.name
			"#,
		)
		.bind(viewer.user_id)
		.fetch_all(pool)
		.await?;

		Ok(rows)
	}

	pub async fn can_user_use_model(pool: &sqlx::PgPool, user_id: &Uuid, model_id: &Uuid) -> Result<bool, sqlx::Error> {
		PolicyResolver::can_use_model(pool, user_id, model_id).await
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

	pub async fn list_by_provider_for_admin(pool: &sqlx::PgPool, provider_id: &Uuid) -> Result<Vec<ProviderModelResponse>, sqlx::Error> {
		let models = sqlx::query_as::<_, Model>(
			r#"
			SELECT
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS capabilities,
				COALESCE(input_modalities, '[]'::jsonb) AS input_modalities,
				COALESCE(output_modalities, '[]'::jsonb) AS output_modalities,
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS is_enabled,
				created_at,
				updated_at
			FROM models
			WHERE provider_id = $1
			ORDER BY display_name
			"#,
		)
		.bind(provider_id)
		.fetch_all(pool)
		.await?;

		Ok(models
			.into_iter()
			.map(|model| ProviderModelResponse {
				id: model.id,
				provider_id: model.provider_id,
				model_id: model.model_id,
				display_name: model.display_name,
				capabilities: model.capabilities.0,
				input_modalities: model.input_modalities.0,
				output_modalities: model.output_modalities.0,
				context_length: model.context_length,
				max_tokens: model.max_tokens,
				is_enabled: model.is_enabled,
			})
			.collect())
	}

	pub async fn sync_provider_models(pool: &sqlx::PgPool, provider_id: &Uuid, discovered: &[ModelSyncInput]) -> Result<ModelSyncSummary, sqlx::Error> {
		let existing = Self::list_raw_by_provider(pool, provider_id).await?;
		let existing_map: BTreeMap<_, _> = existing.iter().map(|model| (model.model_id.as_str(), model)).collect();
		let discovered_ids: HashSet<_> = discovered.iter().map(|model| model.model_id.as_str()).collect();

		let mut to_insert = Vec::with_capacity(discovered.len());
		let mut to_update = Vec::with_capacity(discovered.len());
		let mut to_delete = Vec::with_capacity(existing.len());

		for model in discovered {
			if existing_map.contains_key(model.model_id.as_str()) {
				to_update.push(model);
			} else {
				to_insert.push(model);
			}
		}

		for model in &existing {
			if !discovered_ids.contains(model.model_id.as_str()) {
				to_delete.push(model.id);
			}
		}

		let added = Self::bulk_insert_sync_models(pool, &to_insert).await?;
		let updated = Self::bulk_update_sync_models(pool, &to_update).await?;
		let removed = Self::bulk_delete(pool, &to_delete).await?;

		Ok(ModelSyncSummary { added, updated, removed })
	}

	async fn list_raw_by_provider(pool: &sqlx::PgPool, provider_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS capabilities,
				COALESCE(input_modalities, '[]'::jsonb) AS input_modalities,
				COALESCE(output_modalities, '[]'::jsonb) AS output_modalities,
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS is_enabled,
				created_at,
				updated_at
			FROM models
			WHERE provider_id = $1
			"#,
		)
		.bind(provider_id)
		.fetch_all(pool)
		.await
	}

	async fn bulk_insert_sync_models(pool: &sqlx::PgPool, items: &[&ModelSyncInput]) -> Result<usize, sqlx::Error> {
		if items.is_empty() {
			return Ok(0);
		}

		let mut query_builder = sqlx::QueryBuilder::new(
			"INSERT INTO models (provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, context_length, max_tokens) ",
		);

		query_builder.push_values(items, |mut builder, item| {
			builder
				.push_bind(item.provider_id)
				.push_bind(&item.model_id)
				.push_bind(&item.display_name)
				.push_bind(&item.capabilities)
				.push_bind(&item.input_modalities)
				.push_bind(&item.output_modalities)
				.push_bind(item.context_length)
				.push_bind(item.max_tokens);
		});

		query_builder.build().execute(pool).await?;
		Ok(items.len())
	}

	async fn bulk_update_sync_models(pool: &sqlx::PgPool, items: &[&ModelSyncInput]) -> Result<usize, sqlx::Error> {
		if items.is_empty() {
			return Ok(0);
		}

		let provider_ids: Vec<Uuid> = items.iter().map(|item| item.provider_id).collect();
		let model_ids: Vec<&str> = items.iter().map(|item| item.model_id.as_str()).collect();
		let display_names: Vec<&str> = items.iter().map(|item| item.display_name.as_str()).collect();
		let capabilities: Vec<&Value> = items.iter().map(|item| &item.capabilities).collect();
		let input_modalities: Vec<&Value> = items.iter().map(|item| &item.input_modalities).collect();
		let output_modalities: Vec<&Value> = items.iter().map(|item| &item.output_modalities).collect();
		let context_lengths: Vec<Option<i32>> = items.iter().map(|item| item.context_length).collect();
		let max_tokens: Vec<Option<i32>> = items.iter().map(|item| item.max_tokens).collect();

		sqlx::query(
			r#"
			UPDATE models AS m SET
				display_name = u.display_name,
				capabilities = u.capabilities,
				input_modalities = u.input_modalities,
				output_modalities = u.output_modalities,
				context_length = u.context_length,
				max_tokens = u.max_tokens,
				updated_at = NOW()
			FROM (
				SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::jsonb[], $5::jsonb[], $6::jsonb[], $7::int[], $8::int[])
				AS t(provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, context_length, max_tokens)
			) AS u
			WHERE m.provider_id = u.provider_id AND m.model_id = u.model_id
			"#,
		)
		.bind(&provider_ids)
		.bind(&model_ids)
		.bind(&display_names)
		.bind(&capabilities)
		.bind(&input_modalities)
		.bind(&output_modalities)
		.bind(&context_lengths)
		.bind(&max_tokens)
		.execute(pool)
		.await?;

		Ok(items.len())
	}

	async fn bulk_delete(pool: &sqlx::PgPool, ids: &[Uuid]) -> Result<usize, sqlx::Error> {
		if ids.is_empty() {
			return Ok(0);
		}

		sqlx::query("DELETE FROM models WHERE id = ANY($1)").bind(ids).execute(pool).await?;
		Ok(ids.len())
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
