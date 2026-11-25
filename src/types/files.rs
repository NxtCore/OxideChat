//! Files users store, in the one `files` table: images keep their bytes, documents such as chat attachments keep
//! only their extracted text. Every kind counts toward the same storage quota.

use crate::config::Config;
use sqlx::PgPool;
use uuid::Uuid;

/// The `source` of a document attached to a chat message.
pub const CHAT_ATTACHMENT_SOURCE: &str = "chat_attachment";

/// How much of the storage quota a user's files take up.
pub struct FileStorage;

impl FileStorage {
	/// Bytes the user's files take: originals that were kept, plus extracted text and captions.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn used_bytes(pool: &PgPool, user_id: &Uuid) -> Result<i64, sqlx::Error> {
		sqlx::query_scalar(
			r#"
			SELECT COALESCE(SUM(CASE WHEN data IS NOT NULL OR file_path IS NOT NULL THEN size_bytes ELSE 0 END + COALESCE(octet_length(text), 0)), 0)::bigint
			FROM files WHERE user_id = $1
			"#,
		)
		.bind(user_id)
		.fetch_one(pool)
		.await
	}

	/// Whether the user may store another file under the storage quota an admin set; always true without a quota.
	///
	/// # Errors
	///
	/// Returns a database error when the usage cannot be loaded.
	pub async fn has_room(pool: &PgPool, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let storage = Config::get().storage();
		if storage.quota_mb.is_none() {
			return Ok(true);
		}
		Ok(storage.allows_upload(Self::used_bytes(pool, user_id).await?))
	}
}
