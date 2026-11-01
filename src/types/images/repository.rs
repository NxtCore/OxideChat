use sqlx::PgPool;
use uuid::Uuid;

/// Metadata for an image row that is about to be inserted.
pub struct ImageInsert<'a> {
	pub id: Uuid,
	pub mime_type: &'a str,
	pub size_bytes: i64,
	pub user_id: Option<Uuid>,
	pub source: Option<&'a str>,
}

/// Stored image content: inline bytes or a file path.
#[derive(sqlx::FromRow)]
pub struct ImageContent {
	pub data: Option<Vec<u8>>,
	pub file_path: Option<String>,
	pub mime_type: String,
}

impl ImageInsert<'_> {
	/// Inserts the image with its bytes stored in the database.
	///
	/// # Errors
	///
	/// Returns a database error when the insert fails.
	pub async fn insert_data(&self, pool: &PgPool, data: &[u8]) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
				INSERT INTO images (id, data, mime_type, size_bytes, user_id, source)
				VALUES ($1, $2, $3, $4, $5, $6)
				"#,
		)
		.bind(self.id)
		.bind(data)
		.bind(self.mime_type)
		.bind(self.size_bytes)
		.bind(self.user_id)
		.bind(self.source)
		.execute(pool)
		.await?;
		Ok(())
	}

	/// Inserts the image with its bytes stored in a file.
	///
	/// # Errors
	///
	/// Returns a database error when the insert fails.
	pub async fn insert_file(&self, pool: &PgPool, file_path: &str) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
				INSERT INTO images (id, file_path, mime_type, size_bytes, user_id, source)
				VALUES ($1, $2, $3, $4, $5, $6)
				"#,
		)
		.bind(self.id)
		.bind(file_path)
		.bind(self.mime_type)
		.bind(self.size_bytes)
		.bind(self.user_id)
		.bind(self.source)
		.execute(pool)
		.await?;
		Ok(())
	}
}

impl ImageContent {
	/// Loads the stored content of an image.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT data, file_path, mime_type FROM images WHERE id = $1").bind(id).fetch_optional(pool).await
	}

	/// Stores a caption for an image.
	///
	/// # Errors
	///
	/// Returns a database error when the update fails.
	pub async fn set_caption(pool: &PgPool, id: Uuid, caption: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE images SET caption = $2 WHERE id = $1").bind(id).bind(caption).execute(pool).await?;
		Ok(())
	}

	/// Returns the stored caption of an image, if any.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn caption(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
		let row: Option<(Option<String>,)> = sqlx::query_as("SELECT caption FROM images WHERE id = $1").bind(id).fetch_optional(pool).await?;
		Ok(row.and_then(|r| r.0))
	}
}
