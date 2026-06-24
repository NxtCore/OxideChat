use super::rows::{GatewayCatalogModelRow, GatewayProviderOptionRow};
use super::{
	AvailabilityState, GatewayCatalogModel, GatewayCatalogModelResponse, GatewayCatalogSyncInput, GatewayProviderOptionSyncInput, ModelProviderOptions,
};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use serde_json::Value;
use uuid::Uuid;

/// Resolved target for lazily fetching a runnable model's provider endpoints.
pub struct CatalogEndpointTarget {
	pub catalog_id: Uuid,
	pub provider_id: Uuid,
	pub gateway_model_id: String,
	pub availability_state: AvailabilityState,
}

impl GatewayCatalogModel {
	/// Insert or update the public catalog rows. Availability is set separately by
	/// [`Self::mark_availability`]; new rows default to `AVAILABLE`.
	pub async fn upsert_public_catalog(pool: &sqlx::PgPool, provider_id: &Uuid, source_gateway: &str, items: &[GatewayCatalogSyncInput]) -> Result<(), sqlx::Error> {
		if items.is_empty() {
			return Ok(());
		}

		let ids: Vec<&str> = items.iter().map(|i| i.gateway_model_id.as_str()).collect();
		let names: Vec<&str> = items.iter().map(|i| i.display_name.as_str()).collect();
		let raws: Vec<&Value> = items.iter().map(|i| &i.raw).collect();

		sqlx::query(
			r#"
			INSERT INTO gateway_catalog_models (provider_id, source_gateway, gateway_model_id, display_name, raw, fetched_at)
			SELECT $1, $2, u.gateway_model_id, u.display_name, u.raw, NOW()
			FROM UNNEST($3::text[], $4::text[], $5::jsonb[]) AS u(gateway_model_id, display_name, raw)
			ON CONFLICT (provider_id, source_gateway, gateway_model_id)
			DO UPDATE SET
				display_name = EXCLUDED.display_name,
				raw = EXCLUDED.raw,
				fetched_at = NOW(),
				updated_at = NOW()
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.bind(&ids)
		.bind(&names)
		.bind(&raws)
		.execute(pool)
		.await?;

		Ok(())
	}

	/// Set `availability_state` by diffing the public catalog against the key-scoped
	/// (`/models/user`) set: rows whose `gateway_model_id` is in `user_visible_ids` are
	/// `AVAILABLE`, the rest `USER_UNAVAILABLE`.
	pub async fn mark_availability(pool: &sqlx::PgPool, provider_id: &Uuid, source_gateway: &str, user_visible_ids: &[String]) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			UPDATE gateway_catalog_models
			SET
				availability_state = CASE WHEN gateway_model_id = ANY($3) THEN 'AVAILABLE'::gateway_availability ELSE 'USER_UNAVAILABLE'::gateway_availability END,
				reason = CASE WHEN gateway_model_id = ANY($3) THEN NULL ELSE 'Disabled for this key' END,
				updated_at = NOW()
			WHERE provider_id = $1 AND source_gateway = $2
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.bind(user_visible_ids)
		.execute(pool)
		.await?;

		Ok(())
	}

	/// Delete catalog rows absent from the latest public catalog (cascades to options).
	/// Callers must not pass an empty `public_ids` (that would wipe the whole catalog).
	pub async fn delete_absent(pool: &sqlx::PgPool, provider_id: &Uuid, source_gateway: &str, public_ids: &[String]) -> Result<u64, sqlx::Error> {
		if public_ids.is_empty() {
			return Ok(0);
		}

		let result = sqlx::query(
			r#"
			DELETE FROM gateway_catalog_models
			WHERE provider_id = $1 AND source_gateway = $2 AND gateway_model_id <> ALL($3)
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.bind(public_ids)
		.execute(pool)
		.await?;

		Ok(result.rows_affected())
	}

	/// Refresh `local_model_id` links by mapping `models.model_id` to a bare `author/slug`:
	/// the prefix before the first `/` must match the configured provider name (lowercased);
	/// everything after the first `/` is the bare gateway model id.
	pub async fn refresh_local_model_ids(pool: &sqlx::PgPool, provider_id: &Uuid, source_gateway: &str, provider_name: &str) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			UPDATE gateway_catalog_models
			SET local_model_id = NULL, updated_at = NOW()
			WHERE provider_id = $1 AND source_gateway = $2 AND local_model_id IS NOT NULL
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.execute(pool)
		.await?;

		sqlx::query(
			r#"
			UPDATE gateway_catalog_models gcm
			SET local_model_id = m.id, updated_at = NOW()
			FROM models m
			WHERE gcm.provider_id = $1
			  AND gcm.source_gateway = $2
			  AND m.provider_id = $1
			  AND lower(split_part(m.model_id, '/', 1)) = lower($3)
			  AND gcm.gateway_model_id = substring(m.model_id FROM position('/' IN m.model_id) + 1)
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.bind(provider_name)
		.execute(pool)
		.await?;

		Ok(())
	}

	/// Paginated catalog/search listing for the admin view (includes `USER_UNAVAILABLE`).
	pub async fn list_for_admin(
		pool: &sqlx::PgPool,
		provider_id: &Uuid,
		source_gateway: &str,
		page: i64,
		size: i64,
		search: Option<String>,
	) -> Result<PaginatedResponse<GatewayCatalogModelResponse>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let pattern = search.filter(|s| !s.is_empty()).map(|s| format!("%{}%", escape_like(&s)));

		let rows = sqlx::query_as::<_, GatewayCatalogModelRow>(
			r#"
			SELECT id, gateway_model_id, display_name, availability_state, reason, local_model_id, fetched_at
			FROM gateway_catalog_models
			WHERE provider_id = $1
			  AND source_gateway = $2
			  AND ($3::text IS NULL OR gateway_model_id ILIKE $3 ESCAPE '\' OR display_name ILIKE $3 ESCAPE '\')
			ORDER BY display_name ASC, gateway_model_id ASC
			LIMIT $4 OFFSET $5
			"#,
		)
		.bind(provider_id)
		.bind(source_gateway)
		.bind(&pattern)
		.bind(pagination.limit)
		.bind(pagination.offset)
		.fetch_all(pool)
		.await?;

		Ok(paginate(rows, pagination.page_size))
	}

	/// Provider options for a runnable model, joined through `local_model_id`, plus the
	/// parent availability that drives row render state.
	pub async fn provider_options_for_model(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<ModelProviderOptions, sqlx::Error> {
		let catalog = sqlx::query_as::<_, (Uuid, String, AvailabilityState)>(
			r#"
			SELECT id, gateway_model_id, availability_state
			FROM gateway_catalog_models
			WHERE local_model_id = $1
			LIMIT 1
			"#,
		)
		.bind(model_id)
		.fetch_optional(pool)
		.await?;

		let Some((catalog_id, gateway_model_id, availability)) = catalog else {
			return Ok(ModelProviderOptions {
				gateway_model_id: None,
				availability_state: None,
				options: Vec::new(),
			});
		};

		let rows = sqlx::query_as::<_, GatewayProviderOptionRow>(
			r#"
			SELECT id, provider_slug, provider_name, endpoint_name, status, quantization,
			       context_length, max_completion_tokens, latency, throughput, uptime, price_input, price_output
			FROM gateway_model_provider_options
			WHERE catalog_model_id = $1
			ORDER BY provider_name ASC NULLS LAST, endpoint_name ASC NULLS LAST
			"#,
		)
		.bind(catalog_id)
		.fetch_all(pool)
		.await?;

		Ok(ModelProviderOptions {
			gateway_model_id: Some(gateway_model_id),
			availability_state: Some(availability),
			options: rows.into_iter().map(Into::into).collect(),
		})
	}

	/// Resolve the catalog row backing a runnable model, for lazy endpoint fetching.
	pub async fn find_catalog_for_model(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<Option<CatalogEndpointTarget>, sqlx::Error> {
		let row = sqlx::query_as::<_, (Uuid, Uuid, String, AvailabilityState)>(
			r#"
			SELECT id, provider_id, gateway_model_id, availability_state
			FROM gateway_catalog_models
			WHERE local_model_id = $1
			LIMIT 1
			"#,
		)
		.bind(model_id)
		.fetch_optional(pool)
		.await?;

		Ok(row.map(|(catalog_id, provider_id, gateway_model_id, availability_state)| CatalogEndpointTarget {
			catalog_id,
			provider_id,
			gateway_model_id,
			availability_state,
		}))
	}

	/// Replace the stored provider options for a catalog model with a fresh set.
	pub async fn upsert_provider_options(pool: &sqlx::PgPool, catalog_model_id: &Uuid, items: &[GatewayProviderOptionSyncInput]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;

		sqlx::query("DELETE FROM gateway_model_provider_options WHERE catalog_model_id = $1")
			.bind(catalog_model_id)
			.execute(&mut *tx)
			.await?;

		for item in items {
			sqlx::query(
				r#"
				INSERT INTO gateway_model_provider_options (
					catalog_model_id, provider_slug, provider_name, endpoint_name, status, quantization,
					context_length, max_completion_tokens, latency, throughput, uptime, price_input, price_output, raw, fetched_at
				)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW())
				"#,
			)
			.bind(catalog_model_id)
			.bind(&item.provider_slug)
			.bind(&item.provider_name)
			.bind(&item.endpoint_name)
			.bind(item.status)
			.bind(&item.quantization)
			.bind(item.context_length)
			.bind(item.max_completion_tokens)
			.bind(item.latency)
			.bind(item.throughput)
			.bind(item.uptime)
			.bind(item.price_input)
			.bind(item.price_output)
			.bind(&item.raw)
			.execute(&mut *tx)
			.await?;
		}

		tx.commit().await?;
		Ok(())
	}
}

fn escape_like(input: &str) -> String {
	input.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn paginate<R, T: From<R>>(mut rows: Vec<R>, page_size: usize) -> PaginatedResponse<T> {
	let has_more = rows.len() > page_size;
	rows.truncate(page_size);
	PaginatedResponse {
		has_more,
		items: rows.into_iter().map(Into::into).collect(),
	}
}
