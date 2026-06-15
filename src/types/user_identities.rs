use crate::types::BaseType;
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserIdentity {
	pub id: Uuid,
	pub user_id: Uuid,
	pub provider: String,
	pub provider_user_id: String,
	pub provider_data: Option<Json<Value>>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for UserIdentity {}

impl UserIdentity {
	pub async fn find_by_provider(pool: &sqlx::PgPool, provider: &str, provider_user_id: &str) -> Result<Option<Uuid>, sqlx::Error> {
		let row: Option<(Uuid,)> = sqlx::query_as(
			"SELECT user_id FROM user_identities WHERE provider = $1 AND provider_user_id = $2",
		)
		.bind(provider)
		.bind(provider_user_id)
		.fetch_optional(pool)
		.await?;
		Ok(row.map(|r| r.0))
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		user_id: &Uuid,
		provider: &str,
		provider_user_id: &str,
		provider_data: Option<&Value>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, UserIdentity>(
			r#"
			INSERT INTO user_identities (user_id, provider, provider_user_id, provider_data)
			VALUES ($1, $2, $3, $4)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(provider)
		.bind(provider_user_id)
		.bind(provider_data.map(Json))
		.fetch_one(conn)
		.await
	}

	pub async fn find_by_user_id(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserIdentity>("SELECT * FROM user_identities WHERE user_id = $1 ORDER BY provider")
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM user_identities WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}
}
