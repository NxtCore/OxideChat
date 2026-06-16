use super::Translation;
use super::rows::IdRow;
use uuid::Uuid;

impl Translation {
	pub async fn list_all(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Translation>("SELECT * FROM i18n_translations ORDER BY language, key_path")
			.fetch_all(pool)
			.await
	}

	pub async fn upsert(pool: &sqlx::PgPool, language: &str, key_path: &str, value: &str, is_override: bool) -> Result<Uuid, sqlx::Error> {
		let row: IdRow = sqlx::query_as(
			r#"
			INSERT INTO i18n_translations (language, key_path, value, is_override)
			VALUES ($1, $2, $3, $4)
			ON CONFLICT (language, key_path) DO UPDATE
				SET value = EXCLUDED.value, is_override = EXCLUDED.is_override, updated_at = NOW()
			RETURNING id
			"#,
		)
		.bind(language)
		.bind(key_path)
		.bind(value)
		.bind(is_override)
		.fetch_one(pool)
		.await?;
		Ok(row.id)
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM i18n_translations WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn find_by_language(pool: &sqlx::PgPool, language: &str) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Translation>("SELECT * FROM i18n_translations WHERE language = $1 ORDER BY key_path")
			.bind(language)
			.fetch_all(pool)
			.await
	}
}
