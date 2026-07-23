use super::{Provider, ProviderKind};
use crate::utils::encryption::encrypt_object_values;
use serde_json::Value;
use uuid::Uuid;

impl Provider {
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
		let existing = match Self::find_for_admin(pool, id).await? {
			Some(provider) => provider,
			None => return Ok(None),
		};

		let api_key = api_key.map_or(existing.api_key.as_deref(), |value| value);
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
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
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
