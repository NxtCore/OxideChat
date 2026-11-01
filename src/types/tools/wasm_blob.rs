use super::WasmBlob;
use uuid::Uuid;

impl WasmBlob {
	pub async fn find_by_hash(pool: &sqlx::PgPool, sha256_hash: &str, owner_id: Option<&Uuid>) -> Result<Option<Uuid>, sqlx::Error> {
		let row: Option<(Uuid,)> = if let Some(owner) = owner_id {
			sqlx::query_as("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id = $2")
				.bind(sha256_hash)
				.bind(owner)
				.fetch_optional(pool)
				.await?
		} else {
			sqlx::query_as("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id IS NULL")
				.bind(sha256_hash)
				.fetch_optional(pool)
				.await?
		};
		Ok(row.map(|r| r.0))
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn create(
		pool: &sqlx::PgPool,
		owner_id: Option<&Uuid>,
		original_filename: Option<&str>,
		compiled_from: Option<&str>,
		blob: &[u8],
		size_bytes: i32,
		sha256_hash: &str,
	) -> Result<Uuid, sqlx::Error> {
		let row: (Uuid,) = sqlx::query_as(
			r#"
			INSERT INTO wasm_blobs (owner_id, original_filename, compiled_from, blob, size_bytes, sha256_hash)
			VALUES ($1, $2, $3, $4, $5, $6)
			RETURNING id
			"#,
		)
		.bind(owner_id)
		.bind(original_filename)
		.bind(compiled_from)
		.bind(blob)
		.bind(size_bytes)
		.bind(sha256_hash)
		.fetch_one(pool)
		.await?;
		Ok(row.0)
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Vec<u8>>, sqlx::Error> {
		let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT blob FROM wasm_blobs WHERE id = $1").bind(id).fetch_optional(pool).await?;
		Ok(row.map(|r| r.0))
	}
}
