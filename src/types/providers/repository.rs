use super::Provider;
use uuid::Uuid;

impl Provider {
	pub async fn list_for_admin(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE owner_id IS NULL
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn list_enabled_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE owner_id IS NULL AND is_enabled = true
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn find_for_admin(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE id = $1 AND owner_id IS NULL
			"#,
		)
		.bind(id)
		.fetch_optional(pool)
		.await
	}

	pub async fn create_system(
		pool: &sqlx::PgPool,
		kind: &super::ProviderKind,
		name: &str,
		base_url: &str,
		api_key: Option<&str>,
		extra_headers: &serde_json::Value,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO providers (owner_id, kind, name, base_url, api_key, extra_headers, is_enabled)
			VALUES (NULL, $1, $2, $3, $4, $5, $6)
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			"#,
		)
		.bind(kind)
		.bind(name)
		.bind(base_url)
		.bind(api_key)
		.bind(extra_headers)
		.bind(is_enabled)
		.fetch_one(pool)
		.await
	}

	pub async fn delete_system(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query(
			r#"
			DELETE FROM providers
			WHERE id = $1 AND owner_id IS NULL
			"#,
		)
		.bind(id)
		.execute(pool)
		.await?;

		Ok(result.rows_affected() > 0)
	}
}
