use super::{Provider, ProviderKind, ProviderReportedCost};
use crate::utils::encryption::{encrypt_object_values, keeps_stored_secret};
use crate::utils::outbound::same_origin;
use serde_json::Value;
use uuid::Uuid;

impl Provider {
	/// Sets how this provider reports request cost, or clears it to use token pricing.
	///
	/// # Errors
	/// Returns a database error when the update fails.
	pub async fn set_reported_cost(pool: &sqlx::PgPool, id: &Uuid, reported_cost: Option<&ProviderReportedCost>) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			UPDATE providers SET reported_cost = $2, updated_at = NOW()
			WHERE id = $1 AND owner_id IS NULL
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, reported_cost, created_at, updated_at
			"#,
		)
		.bind(id)
		.bind(reported_cost.map(sqlx::types::Json))
		.fetch_optional(pool)
		.await
	}

	/// Returns `true` when a system provider reports its cost in a currency other than US dollars.
	///
	/// # Errors
	/// Returns a database error when the query fails.
	pub async fn any_reports_cost_in_currency(pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM providers WHERE owner_id IS NULL AND reported_cost->>'unit' = 'currency')")
			.fetch_one(pool)
			.await
	}

	/// Whether an update would send this provider's stored key or headers to another host.
	///
	/// True when `base_url` names a different origin and the key is not replaced (`new_key`), or
	/// the headers are left as they are or still contain masked values.
	#[must_use]
	pub fn redirects_secrets(&self, base_url: Option<&str>, new_key: bool, extra_headers: Option<&Value>) -> bool {
		let Some(base_url) = base_url else {
			return false;
		};
		if self.base_url == base_url || same_origin(&self.base_url, base_url) {
			return false;
		}
		let keeps_key = self.api_key.as_deref().is_some_and(|key| !key.is_empty()) && !new_key;
		let headers = extra_headers.unwrap_or(&self.extra_headers.0);
		let keeps_headers = match (extra_headers, headers.as_object()) {
			(_, None) => false,
			(None, Some(stored)) => !stored.is_empty(),
			(Some(_), Some(sent)) => sent.values().any(|value| value.as_str().is_some_and(keeps_stored_secret)),
		};
		keeps_key || keeps_headers
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
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, reported_cost, created_at, updated_at
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
