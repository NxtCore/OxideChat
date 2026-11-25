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
	pub async fn insert_data<'e, E: sqlx::PgExecutor<'e>>(&self, pool: E, data: &[u8]) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
				INSERT INTO files (id, kind, data, mime_type, size_bytes, user_id, source)
				VALUES ($1, 'image', $2, $3, $4, $5, $6)
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
	pub async fn insert_file<'e, E: sqlx::PgExecutor<'e>>(&self, pool: E, file_path: &str) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
				INSERT INTO files (id, kind, file_path, mime_type, size_bytes, user_id, source)
				VALUES ($1, 'image', $2, $3, $4, $5, $6)
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
		sqlx::query_as("SELECT data, file_path, mime_type FROM files WHERE id = $1 AND kind = 'image'").bind(id).fetch_optional(pool).await
	}

	/// Loads an image only when the user may read it: their own images and icons, which every signed-in user sees.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find_for_user(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT data, file_path, mime_type FROM files WHERE id = $1 AND kind = 'image' AND (user_id = $2 OR source IN ('icon', 'model_icon'))")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	/// Stores a caption for an image.
	///
	/// # Errors
	///
	/// Returns a database error when the update fails.
	pub async fn set_caption(pool: &PgPool, id: Uuid, caption: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE files SET text = $2 WHERE id = $1 AND kind = 'image'").bind(id).bind(caption).execute(pool).await?;
		Ok(())
	}

	/// Returns the stored caption of one of the user's images, if any.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn caption(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<String>, sqlx::Error> {
		let row: Option<(Option<String>,)> = sqlx::query_as("SELECT text FROM files WHERE id = $1 AND kind = 'image' AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await?;
		Ok(row.and_then(|r| r.0))
	}
}
