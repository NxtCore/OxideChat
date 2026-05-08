use crate::types::BaseType;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct RateLimit {
	pub id: Uuid,
	pub endpoint_pattern: String,
	pub requests_per_window: i32,
	pub window_seconds: i32,
	pub scope: String,
	pub enabled: bool,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for RateLimit {
	const TABLE: &'static str = "rate_limits";
	const ALIAS: &'static str = "rl";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			endpoint_pattern: String::new(),
			requests_per_window: 0,
			window_seconds: 0,
			scope: String::from("ip"),
			enabled: true,
			created_at: chrono::Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id",
			"endpoint_pattern",
			"requests_per_window",
			"window_seconds",
			"scope",
			"enabled",
			"created_at",
		]
	}
}

impl RateLimit {
	pub async fn find_by_endpoint(pool: &sqlx::PgPool, endpoint_pattern: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, RateLimit>("SELECT * FROM rate_limits WHERE endpoint_pattern = $1")
			.bind(endpoint_pattern)
			.fetch_optional(pool)
			.await
	}

	pub async fn list_all_enabled(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, RateLimit>("SELECT * FROM rate_limits WHERE enabled = true ORDER BY endpoint_pattern")
			.fetch_all(pool)
			.await
	}

	pub async fn list_all(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, RateLimit>("SELECT * FROM rate_limits ORDER BY endpoint_pattern")
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, RateLimit>("SELECT * FROM rate_limits WHERE id = $1")
			.bind(id)
			.fetch_optional(pool)
			.await
	}
}
