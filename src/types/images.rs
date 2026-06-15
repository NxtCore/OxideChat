use crate::types::BaseType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct Image {
	pub id: Uuid,
	pub data: Option<Vec<u8>>,
	pub file_path: Option<String>,
	pub mime_type: String,
	pub size_bytes: i64,
	pub source: Option<String>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for Image {}

impl Image {
	pub async fn create_from_bytes(
		pool: &sqlx::PgPool,
		data: &[u8],
		mime_type: &str,
		size_bytes: i64,
		source: Option<&str>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Image>(
			r#"
			INSERT INTO images (data, mime_type, size_bytes, source)
			VALUES ($1, $2, $3, $4)
			RETURNING *
			"#,
		)
		.bind(data)
		.bind(mime_type)
		.bind(size_bytes)
		.bind(source)
		.fetch_one(pool)
		.await
	}

	pub async fn create_from_file(
		pool: &sqlx::PgPool,
		file_path: &str,
		mime_type: &str,
		size_bytes: i64,
		source: Option<&str>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Image>(
			r#"
			INSERT INTO images (file_path, mime_type, size_bytes, source)
			VALUES ($1, $2, $3, $4)
			RETURNING *
			"#,
		)
		.bind(file_path)
		.bind(mime_type)
		.bind(size_bytes)
		.bind(source)
		.fetch_one(pool)
		.await
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Image>("SELECT * FROM images WHERE id = $1")
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM images WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}
}

#[derive(Debug, Deserialize)]
pub struct UploadImageRequest {
	pub data_uri: String,
	pub user_id: Option<Uuid>,
	pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
	pub id: Uuid,
	pub url: String,
	pub mime_type: String,
	pub size_bytes: i64,
}
