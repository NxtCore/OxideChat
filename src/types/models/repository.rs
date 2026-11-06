use super::rows::{ModelDetailedRow, ModelListAdminRow, ModelListPublicRow, ModelPricingRow};
use super::{Model, ModelDetailed, ModelListAdmin, ModelListParams, ModelListPublic, ModelPricing, ModelSyncInput, ModelSyncSummary, ModelViewer};
use crate::types::BaseType;
use crate::types::PolicyResolver;
use crate::types::UsageEventRecord;
use crate::types::{Page, PageQuery};
use crate::types::models::ProviderTab;
use crate::types::providers::{ProviderKind, ProviderModelResponse};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::{BTreeMap, HashMap, HashSet};
use omniference::catalog::Catalog;
use serde::Deserialize;
use uuid::Uuid;

impl Model {
	pub async fn list_image_providers_for_admin(pool: &sqlx::PgPool) -> Result<Vec<ProviderTab>, sqlx::Error> {
		sqlx::query_as::<_, ProviderTab>(
			r#"
			SELECT DISTINCT p.id, p.name
			FROM providers p
			JOIN models m ON m.provider_id = p.id
			WHERE p.is_enabled = true
			  AND p.kind IN ('OPENAI', 'OPENROUTER', 'GOOGLE')
			  AND m.is_enabled = true
			  AND EXISTS (
				  SELECT 1
				  FROM jsonb_array_elements_text(COALESCE(m.output_modalities, '[]'::jsonb)) AS modality(value)
				  WHERE LOWER(modality.value) = 'image'
			  )
			ORDER BY p.name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn list_image_models_for_admin(
		pool: &sqlx::PgPool,
		page: &PageQuery,
		search_query: Option<String>,
		provider_id: Option<Uuid>,
	) -> Result<Page<ModelListAdmin>, sqlx::Error> {
		let pagination = Self::pagination(page);
		let search = search_query
			.as_deref()
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.map(|value| format!("%{}%", Self::escape_like_pattern(value)));
		let rows = sqlx::query_as::<_, ModelListAdminRow>(
			r#"
			SELECT m.id, m.model_id, m.display_name,
			       COALESCE(m.capabilities, '[]'::jsonb) AS capabilities,
			       COALESCE(m.input_modalities, '[]'::jsonb) AS input_modalities,
			       COALESCE(m.output_modalities, '[]'::jsonb) AS output_modalities,
			       m.context_length, m.max_tokens, COALESCE(m.is_enabled, false) AS is_enabled,
			       m.created_at, m.updated_at, p.id AS provider_id, p.name AS provider_name,
			       p.kind AS provider_kind, NULL::text AS icon
			FROM models m
			JOIN providers p ON p.id = m.provider_id
			WHERE m.is_enabled = true AND p.is_enabled = true
			  AND EXISTS (
				  SELECT 1
				  FROM jsonb_array_elements_text(COALESCE(m.output_modalities, '[]'::jsonb)) AS modality(value)
				  WHERE LOWER(modality.value) = 'image'
			  )
			  AND ($3::text IS NULL OR m.display_name ILIKE $3 ESCAPE '\' OR m.model_id ILIKE $3 ESCAPE '\' OR p.name ILIKE $3 ESCAPE '\')
			  AND ($4::uuid IS NULL OR p.id = $4)
			ORDER BY m.display_name, p.name
			LIMIT $1 OFFSET $2
			"#,
		)
		.bind(pagination.limit)
		.bind(pagination.offset)
		.bind(search)
		.bind(provider_id)
		.fetch_all(pool)
		.await?;
		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(ModelListAdmin::from).collect();
		Ok(Page { has_more, items })
	}
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
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

	pub async fn find_by_model_id(pool: &sqlx::PgPool, model_id: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Model>(
			r#"
			SELECT id, provider_id, model_id, display_name, capabilities, input_modalities,
			       output_modalities, context_length, max_tokens, is_enabled, created_at, updated_at
			FROM models
			WHERE model_id = $1
			"#,
		)
		.bind(model_id)
		.fetch_optional(pool)
		.await
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn list_for_user(
		pool: &sqlx::PgPool,
		viewer: ModelViewer<'_>,
		page: &PageQuery,
		show_disabled: bool,
		search_query: Option<&str>,
		favorites_only: bool,
		provider_id: Option<&Uuid>,
	) -> Result<Page<ModelListPublic>, sqlx::Error> {
		let pagination = Self::pagination(page);
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

		Ok(Page { has_more, items })
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

	/// Lists models for admins, filtered by search text, provider and output modality.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_for_admin(pool: &sqlx::PgPool, page: &PageQuery, params: &ModelListParams) -> Result<Page<ModelListAdmin>, sqlx::Error> {
		let pagination = Self::pagination(page);
		let search = params
			.query
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
			AND ($4::UUID IS NULL OR p.id = $4)
			AND (
				$5::TEXT IS NULL
				OR (
					jsonb_array_length(COALESCE(sys_mc.output_modalities, m.output_modalities, '[]'::jsonb)) > 0
					AND NOT EXISTS (
						SELECT 1
						FROM jsonb_array_elements_text(COALESCE(sys_mc.output_modalities, m.output_modalities, '[]'::jsonb)) AS modality(value)
						WHERE LOWER(modality.value) <> LOWER($5)
					)
				)
			)
			ORDER BY m.display_name, p.name
			LIMIT $1 OFFSET $2
			"#,
			pagination.limit,
			pagination.offset,
			search.as_deref(),
			params.provider_id,
			params.output_only.as_deref(),
		)
		.fetch_all(pool)
		.await?;

		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(ModelListAdmin::from).collect();

		Ok(Page { has_more, items })
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
		let mut transaction = pool.begin().await?;
		let existing = Self::list_raw_by_provider(&mut transaction, provider_id).await?;
		let existing_map: BTreeMap<_, _> = existing.iter().map(|model| (model.model_id.as_str(), model)).collect();
		let discovered_ids: HashSet<_> = discovered.iter().map(|model| model.model_id.as_str()).collect();

		let mut to_insert = Vec::with_capacity(discovered.len());
		let mut to_update = Vec::with_capacity(discovered.len());
		let mut to_disable = Vec::with_capacity(existing.len());

		for model in discovered {
			if existing_map.contains_key(model.model_id.as_str()) {
				to_update.push(model);
			} else {
				to_insert.push(model);
			}
		}

		for model in &existing {
			if !discovered_ids.contains(model.model_id.as_str()) {
				to_disable.push(model.id);
			}
		}

		let added = Self::bulk_insert_sync_models(&mut transaction, &to_insert).await?;
		let updated = Self::bulk_update_sync_models(&mut transaction, &to_update).await?;
		let removed = Self::bulk_disable_sync_models(&mut transaction, &to_disable).await?;
		transaction.commit().await?;

		Ok(ModelSyncSummary { added, updated, removed })
	}

	async fn list_raw_by_provider(connection: &mut sqlx::PgConnection, provider_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
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
		.fetch_all(connection)
		.await
	}

	async fn bulk_insert_sync_models(connection: &mut sqlx::PgConnection, items: &[&ModelSyncInput]) -> Result<usize, sqlx::Error> {
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

		query_builder.build().execute(connection).await?;
		Ok(items.len())
	}

	async fn bulk_update_sync_models(connection: &mut sqlx::PgConnection, items: &[&ModelSyncInput]) -> Result<usize, sqlx::Error> {
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
				is_enabled = true,
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
		.execute(connection)
		.await?;

		Ok(items.len())
	}

	async fn bulk_disable_sync_models(connection: &mut sqlx::PgConnection, ids: &[Uuid]) -> Result<usize, sqlx::Error> {
		if ids.is_empty() {
			return Ok(0);
		}

		let result = sqlx::query("UPDATE models SET is_enabled = false, updated_at = NOW() WHERE id = ANY($1) AND is_enabled = true")
			.bind(ids)
			.execute(connection)
			.await?;
		Ok(result.rows_affected() as usize)
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

impl ModelPricing {
	fn decimal_from_json(value: Option<&Value>) -> Option<Decimal> {
		value.and_then(Value::as_f64).and_then(Decimal::from_f64_retain)
	}

	async fn rows(pool: &sqlx::PgPool, model_id: Option<&Uuid>) -> Result<Vec<ModelPricingRow>, sqlx::Error> {
		sqlx::query_as::<_, ModelPricingRow>(
			r#"
			SELECT m.id AS model_id, m.model_id AS provider_model_id, p.name AS provider_name, p.kind AS provider_kind, mpo.pricing AS override_pricing
			FROM models m
			JOIN providers p ON p.id = m.provider_id
			LEFT JOIN model_pricing_overrides mpo ON mpo.model_id = m.id
			WHERE $1::uuid IS NULL OR m.id = $1
			"#,
		)
		.bind(model_id)
		.fetch_all(pool)
		.await
	}

	/// The catalog's input and output price for a provider's model, in USD per million tokens.
	/// `provider_model_id` is the stored `provider/model` ID.
	pub async fn catalog_rates(catalog: &Catalog, provider_name: &str, kind: &ProviderKind, provider_model_id: &str) -> (Option<Decimal>, Option<Decimal>) {
		Self::rates(Self::catalog_price(catalog, provider_name, kind, provider_model_id).await.as_ref())
	}

	async fn catalog_price(catalog: &Catalog, provider_name: &str, kind: &ProviderKind, provider_model_id: &str) -> Option<omniference::catalog::ModelPricing> {
		let native_model_id = provider_model_id.split_once('/').map_or(provider_model_id, |(_, id)| id);
		catalog.catalog_pricing(&crate::ai::catalog_provider(provider_name, kind), native_model_id).await
	}

	fn rates(pricing: Option<&omniference::catalog::ModelPricing>) -> (Option<Decimal>, Option<Decimal>) {
		match pricing {
			Some(pricing) => (Decimal::from_f64(pricing.input), Decimal::from_f64(pricing.output)),
			None => (None, None),
		}
	}

	async fn from_row(catalog: &Catalog, row: ModelPricingRow) -> Self {
		let catalog_price = Self::catalog_price(catalog, &row.provider_name, &row.provider_kind, &row.provider_model_id).await;
		let (reported_input, reported_output) = Self::rates(catalog_price.as_ref());
		let override_rate = |key: &str| row.override_pricing.as_ref().and_then(|value| Self::decimal_from_json(value.get(key)));
		let effective_input = override_rate("input").or(reported_input).unwrap_or(Decimal::ZERO);
		let effective_output = override_rate("output").or(reported_output).unwrap_or(Decimal::ZERO);
		let manual_price = row
			.override_pricing
			.as_ref()
			.and_then(|value| omniference::catalog::ModelPricing::deserialize(value).ok());
		Self {
			model_id: row.model_id,
			reported_input,
			reported_output,
			override_pricing: row.override_pricing,
			effective_input,
			effective_output,
			pricing: manual_price.or(catalog_price),
		}
	}

	/// The model's price: the manual price when set, else the catalog price Omniference knows for
	/// the model's provider (models.dev and the catalog overrides).
	///
	/// # Errors
	/// Returns an error when the model cannot be loaded.
	pub async fn effective(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		let Some(row) = Self::rows(pool, Some(model_id)).await?.pop() else {
			return Ok(None);
		};
		Ok(Some(Self::from_row(&crate::ai::catalog(), row).await))
	}

	/// Every manual price as `(provider name, model ID, pricing)`, the keys Omniference prices requests by.
	///
	/// # Errors
	/// Returns an error when the prices cannot be loaded.
	pub async fn engine_overrides(pool: &sqlx::PgPool) -> Result<Vec<(String, String, omniference::catalog::ModelPricing)>, sqlx::Error> {
		let rows = sqlx::query_as::<_, ModelPricingRow>(
			r#"
			SELECT m.id AS model_id, m.model_id AS provider_model_id, p.name AS provider_name, p.kind AS provider_kind, mpo.pricing AS override_pricing
			FROM model_pricing_overrides mpo
			JOIN models m ON m.id = mpo.model_id
			JOIN providers p ON p.id = m.provider_id
			"#,
		)
		.fetch_all(pool)
		.await?;
		Ok(rows
			.iter()
			.filter_map(|row| {
				let pricing = omniference::catalog::ModelPricing::deserialize(row.override_pricing.as_ref()?).ok()?;
				Some((row.provider_name.clone(), row.native_model_id().to_string(), pricing))
			})
			.collect())
	}

	/// Sends the model's manual price to the running engine, or removes it there when none is stored.
	///
	/// # Errors
	/// Returns an error when the model cannot be loaded.
	pub async fn push_to_engine(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<(), sqlx::Error> {
		let Some(row) = Self::rows(pool, Some(model_id)).await?.pop() else {
			return Ok(());
		};
		let catalog = crate::ai::catalog();
		let pricing = row
			.override_pricing
			.as_ref()
			.and_then(|value| omniference::catalog::ModelPricing::deserialize(value).ok());
		match pricing {
			Some(pricing) => catalog.set_pricing_override(&row.provider_name, row.native_model_id(), pricing).await,
			None => catalog.clear_pricing_override(&row.provider_name, row.native_model_id()).await,
		}
		Ok(())
	}

	pub async fn upsert_override(pool: &sqlx::PgPool, model_id: &Uuid, pricing: &Value) -> Result<Self, sqlx::Error> {
		Self::store_override(pool, model_id, pricing).await?;
		Self::push_to_engine(pool, model_id).await?;
		Self::effective(pool, model_id).await?.ok_or(sqlx::Error::RowNotFound)
	}

	/// Stores a pricing override, inside the caller's transaction when given one. Call
	/// [`ModelPricing::push_to_engine`] once it is committed.
	///
	/// # Errors
	///
	/// Returns an error when the override cannot be stored.
	pub async fn store_override<'e, E: sqlx::PgExecutor<'e>>(executor: E, model_id: &Uuid, pricing: &Value) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			INSERT INTO model_pricing_overrides (model_id, pricing)
			VALUES ($1, $2)
			ON CONFLICT (model_id) DO UPDATE
			SET pricing = EXCLUDED.pricing,
			    updated_at = NOW()
			"#,
		)
		.bind(model_id)
		.bind(pricing)
		.execute(executor)
		.await?;
		Ok(())
	}

	pub async fn delete_override(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM model_pricing_overrides WHERE model_id = $1")
			.bind(model_id)
			.execute(pool)
			.await?;
		Self::push_to_engine(pool, model_id).await
	}

	/// Sets `record.cost_total`: the provider-reported cost when present, else the model's price, else `fallback_cost`.
	///
	/// # Errors
	/// Returns an error when the price cannot be loaded.
	pub async fn price(pool: &sqlx::PgPool, provider_cost: Option<f64>, fallback_cost: f64, record: &mut UsageEventRecord<'_>) -> Result<(), sqlx::Error> {
		record.cost_total = match provider_cost.and_then(Decimal::from_f64) {
			Some(cost) => cost,
			None => Self::usage_cost(pool, record.model_id, record.input_tokens, record.output_tokens, record.reasoning_tokens)
				.await?
				.unwrap_or_else(|| Decimal::from_f64(fallback_cost).unwrap_or(Decimal::ZERO)),
		};
		Ok(())
	}

	fn has_price(&self) -> bool {
		self.override_pricing.is_some() || (self.reported_input.is_some() && self.reported_output.is_some())
	}

	fn reasoning_rate(&self) -> Option<Decimal> {
		self.override_pricing.as_ref().and_then(|value| Self::decimal_from_json(value.get("reasoning")))
	}

	async fn usage_cost(pool: &sqlx::PgPool, model_id: &Uuid, input_tokens: i32, output_tokens: i32, reasoning_tokens: i32) -> Result<Option<Decimal>, sqlx::Error> {
		let Some(pricing) = Self::effective(pool, model_id).await?.and_then(|pricing| pricing.pricing) else {
			return Ok(None);
		};
		let tokens = |count: i32| u32::try_from(count).unwrap_or(0);
		let usage = omniference::catalog::UsageBreakdown {
			input_tokens: tokens(input_tokens),
			output_tokens: tokens(output_tokens),
			reasoning_tokens: tokens(reasoning_tokens),
			..Default::default()
		};
		Ok(omniference::catalog::compute_cost(Some(&pricing), &usage)
			.ok()
			.and_then(|cost| Decimal::from_f64(cost.total)))
	}

	/// Prices an estimate for a model; unpriced models estimate to zero.
	///
	/// # Errors
	/// Returns an error when pricing cannot be loaded.
	pub async fn estimate_cost(pool: &sqlx::PgPool, model_id: &Uuid, tokens: crate::types::TokenEstimate) -> Result<Decimal, sqlx::Error> {
		let clamp = |value: u64| i32::try_from(value).unwrap_or(i32::MAX);
		Ok(Self::usage_cost(pool, model_id, clamp(tokens.input), clamp(tokens.output), 0).await?.unwrap_or(Decimal::ZERO))
	}

	fn charges(&self) -> bool {
		self.effective_input > Decimal::ZERO || self.effective_output > Decimal::ZERO || self.reasoning_rate().is_some_and(|rate| rate > Decimal::ZERO)
	}

	/// Whether a known price is zero; models without a price are not free.
	///
	/// # Errors
	/// Returns an error when pricing cannot be loaded.
	pub async fn is_free(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<bool, sqlx::Error> {
		Ok(Self::effective(pool, model_id).await?.is_some_and(|pricing| pricing.has_price() && !pricing.charges()))
	}

	/// Models that cost money, plus models without a known price, which budgets treat as priced.
	///
	/// # Errors
	/// Returns an error when the models cannot be loaded.
	pub async fn priced_model_ids(pool: &sqlx::PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
		let rows = Self::rows(pool, None).await?;
		let catalog = crate::ai::catalog();
		let mut priced = Vec::with_capacity(rows.len());
		for row in rows {
			let pricing = Self::from_row(&catalog, row).await;
			if !pricing.has_price() || pricing.charges() {
				priced.push(pricing.model_id);
			}
		}
		Ok(priced)
	}
}
