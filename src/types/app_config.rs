use crate::types::BaseType;
use serde_json::Value;

#[derive(Debug, sqlx::FromRow)]
pub struct AppConfig {
	pub key: String,
	pub value: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for AppConfig {
	const TABLE: &'static str = "app_config";
	const ALIAS: &'static str = "ac";

	fn new() -> Self {
		Self {
			key: String::new(),
			value: String::new(),
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&["key", "value", "created_at", "updated_at"]
	}
}

impl AppConfig {
	pub async fn get(pool: &sqlx::PgPool, key: &str) -> Result<Option<String>, sqlx::Error> {
		let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_config WHERE key = $1").bind(key).fetch_optional(pool).await?;
		Ok(row.map(|r| r.0))
	}

	pub async fn set(pool: &sqlx::PgPool, key: &str, value: &str) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			INSERT INTO app_config (key, value)
			VALUES ($1, $2)
			ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
			"#,
		)
		.bind(key)
		.bind(value)
		.execute(pool)
		.await?;
		Ok(())
	}

	pub async fn get_json(pool: &sqlx::PgPool, key: &str) -> Result<Option<Value>, sqlx::Error> {
		let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_config WHERE key = $1").bind(key).fetch_optional(pool).await?;
		row.map(|r| serde_json::from_str(&r.0).map_err(|e| sqlx::Error::Protocol(e.to_string().into())))
			.transpose()
	}

	pub async fn get_all(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, AppConfig>("SELECT * FROM app_config ORDER BY key").fetch_all(pool).await
	}

	pub async fn exists(pool: &sqlx::PgPool, key: &str) -> Result<bool, sqlx::Error> {
		let row: Option<(String,)> = sqlx::query_as("SELECT 1 FROM app_config WHERE key = $1").bind(key).fetch_optional(pool).await?;
		Ok(row.is_some())
	}
}
