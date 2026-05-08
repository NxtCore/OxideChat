use crate::types::BaseType;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct Session {
	pub id: Uuid,
	pub user_id: Uuid,
	pub expires_at: chrono::DateTime<chrono::Utc>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for Session {
	const TABLE: &'static str = "sessions";
	const ALIAS: &'static str = "s";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			user_id: Uuid::new_v4(),
			expires_at: chrono::Utc::now(),
			created_at: chrono::Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&["id", "user_id", "expires_at", "created_at"]
	}
}

impl Session {
	pub async fn create(pool: &sqlx::PgPool, user_id: &Uuid, expires_at: &chrono::DateTime<chrono::Utc>) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Session>(
			r#"
			INSERT INTO sessions (user_id, expires_at)
			VALUES ($1, $2)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(expires_at)
		.fetch_one(pool)
		.await
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Session>, sqlx::Error> {
		sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	pub async fn find_user_id_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Uuid>, sqlx::Error> {
		let row: Option<(Uuid,)> = sqlx::query_as("SELECT user_id FROM sessions WHERE id = $1")
			.bind(id)
			.fetch_optional(pool)
			.await?;
		Ok(row.map(|r| r.0))
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM sessions WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn delete_by_user_id(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(user_id).execute(pool).await?;
		Ok(result.rows_affected())
	}

	pub async fn delete_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()").execute(pool).await?;
		Ok(result.rows_affected())
	}

	pub async fn find_by_user_id(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE user_id = $1 ORDER BY created_at DESC")
			.bind(user_id)
			.fetch_all(pool)
			.await
	}
}
