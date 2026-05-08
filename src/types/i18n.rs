use crate::types::BaseType;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Translation {
	pub id: sqlx::types::Uuid,
	pub language: String,
	pub key_path: String,
	pub value: String,
	pub is_override: bool,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for Translation {
	const TABLE: &'static str = "i18n_translations";
	const ALIAS: &'static str = "i18n";

	fn new() -> Self {
		Self {
			id: sqlx::types::Uuid::new_v4(),
			language: String::new(),
			key_path: String::new(),
			value: String::new(),
			is_override: false,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id", "language", "key_path", "value", "is_override",
			"created_at", "updated_at",
		]
	}
}

impl Translation {
	pub async fn list_all(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Translation>(
			"SELECT * FROM i18n_translations ORDER BY language, key_path",
		)
		.fetch_all(pool)
		.await
	}

	pub async fn upsert(
		conn: &mut sqlx::PgConnection,
		language: &str,
		key_path: &str,
		value: &str,
		is_override: bool,
	) -> Result<sqlx::types::Uuid, sqlx::Error> {
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
		.fetch_one(conn)
		.await?;
		Ok(row.id)
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &sqlx::types::Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM i18n_translations WHERE id = $1")
			.bind(id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn find_by_language(pool: &sqlx::PgPool, language: &str) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Translation>(
			"SELECT * FROM i18n_translations WHERE language = $1 ORDER BY key_path",
		)
		.bind(language)
		.fetch_all(pool)
		.await
	}
}

#[derive(Debug, Deserialize)]
pub struct UpsertTranslationRequest {
	pub language: String,
	pub key_path: String,
	pub value: String,
	#[serde(default)]
	pub is_override: bool,
}

#[derive(Debug, Serialize)]
pub struct TranslationsResponse {
	pub translations: Vec<Translation>,
}

#[derive(FromRow)]
pub struct IdRow {
	pub id: sqlx::types::Uuid,
}
