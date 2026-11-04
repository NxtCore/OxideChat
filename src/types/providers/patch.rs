use super::{Provider, ProviderKind};
use crate::utils::encryption::encrypt_object_values;
use serde_json::Value;
use uuid::Uuid;

impl Provider {
	/// Sets the factor applied to the cost this provider reports, or clears it.
	///
	/// # Errors
	/// Returns a database error when the update fails.
	pub async fn set_cost_conversion(pool: &sqlx::PgPool, id: &Uuid, factor: Option<rust_decimal::Decimal>) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			UPDATE providers SET cost_conversion_factor = $2, updated_at = NOW()
			WHERE id = $1 AND owner_id IS NULL
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, cost_conversion_factor, created_at, updated_at
			"#,
		)
		.bind(id)
		.bind(factor)
		.fetch_optional(pool)
		.await
	}

	/// IDs of providers whose reported cost is used: those with a factor of their own or on one of their models.
	///
	/// # Errors
	/// Returns a database error when the query fails.
	pub async fn ids_with_cost_conversion(pool: &sqlx::PgPool) -> Result<std::collections::HashSet<Uuid>, sqlx::Error> {
		let ids: Vec<Uuid> = sqlx::query_scalar(
			r#"
			SELECT id FROM providers WHERE cost_conversion_factor IS NOT NULL
			UNION
			SELECT provider_id FROM models WHERE cost_conversion_factor IS NOT NULL
			"#,
		)
		.fetch_all(pool)
		.await?;
		Ok(ids.into_iter().collect())
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn patch_system(
		pool: &sqlx::PgPool,
		id: &Uuid,
		kind: Option<&ProviderKind>,
		name: Option<&str>,
		base_url: Option<&str>,
		api_key: Option<Option<&str>>,
		extra_headers: Option<&Value>,
		is_enabled: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		let Some(existing) = Self::find_for_admin(pool, id).await? else {
			return Ok(None);
		};

		let api_key = api_key.unwrap_or(existing.api_key.as_deref());
		let protected_headers = extra_headers
			.map(|headers| encrypt_object_values(headers, Some(&existing.extra_headers.0)))
			.transpose()
			.map_err(|error| sqlx::Error::Protocol(error.to_string()))?;

		sqlx::query_as::<_, Self>(
			r#"
			UPDATE providers
			SET
				kind = COALESCE($2, kind),
				name = COALESCE($3, name),
				base_url = COALESCE($4, base_url),
				api_key = $5,
				extra_headers = COALESCE($6, extra_headers),
				is_enabled = COALESCE($7, is_enabled),
				updated_at = NOW()
			WHERE id = $1 AND owner_id IS NULL
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, cost_conversion_factor, created_at, updated_at
			"#,
		)
		.bind(id)
		.bind(kind)
		.bind(name)
		.bind(base_url)
		.bind(api_key)
		.bind(protected_headers.as_ref())
		.bind(is_enabled)
		.fetch_optional(pool)
		.await
	}
}
