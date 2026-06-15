use crate::types::BaseType;
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct Usage {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub provider_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub request_type: String,
	pub input_tokens: Option<i32>,
	pub output_tokens: Option<i32>,
	pub total_tokens: Option<i32>,
	pub latency_ms: Option<i32>,
	pub success: Option<bool>,
	pub error_message: Option<String>,
	pub metadata: Json<Value>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for Usage {}

impl Usage {
	pub async fn create(
		pool: &sqlx::PgPool,
		user_id: Option<&Uuid>,
		provider_id: Option<&Uuid>,
		model_id: Option<&Uuid>,
		request_type: &str,
		input_tokens: Option<i32>,
		output_tokens: Option<i32>,
		total_tokens: Option<i32>,
		latency_ms: Option<i32>,
		success: Option<bool>,
		error_message: Option<&str>,
		metadata: Option<&Value>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Usage>(
			r#"
			INSERT INTO usage (user_id, provider_id, model_id, request_type,
			                   input_tokens, output_tokens, total_tokens, latency_ms,
			                   success, error_message, metadata)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(provider_id)
		.bind(model_id)
		.bind(request_type)
		.bind(input_tokens)
		.bind(output_tokens)
		.bind(total_tokens)
		.bind(latency_ms)
		.bind(success)
		.bind(error_message)
		.bind(metadata.map(|v| Json(v)))
		.fetch_one(pool)
		.await
	}

	pub async fn find_by_user_id(pool: &sqlx::PgPool, user_id: &Uuid, limit: i64, offset: i64) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Usage>(
			"SELECT * FROM usage WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
		)
		.bind(user_id)
		.bind(limit)
		.bind(offset)
		.fetch_all(pool)
		.await
	}

	pub async fn total_tokens_by_user(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Option<i64>, sqlx::Error> {
		let row: Option<(i64,)> = sqlx::query_as(
			"SELECT COALESCE(SUM(total_tokens), 0) FROM usage WHERE user_id = $1",
		)
		.bind(user_id)
		.fetch_optional(pool)
		.await?;
		Ok(row.map(|r| r.0))
	}
}
