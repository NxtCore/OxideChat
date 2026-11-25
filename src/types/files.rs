//! Files users store, in the one `files` table: images keep their bytes, documents such as chat attachments keep
//! only their extracted text. Every kind is uploaded through `POST /api/v1/files` and counts toward the same storage quota.

use crate::config::Config;
use crate::types::ChatAttachment;
use crate::utils::extract::MAX_UPLOAD_BYTES;
use crate::utils::images::{detect_image_mime, image_url, store_image_in};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// The `source` of a document attached to a chat message.
pub const CHAT_ATTACHMENT_SOURCE: &str = "chat_attachment";

/// The `source` of an image a user uploaded to a chat message.
pub const IMAGE_UPLOAD_SOURCE: &str = "upload";

/// The `source` of an image uploaded as an icon, which every signed-in user may see.
pub const ICON_SOURCE: &str = "icon";

/// Most files one user may upload per hour.
pub const UPLOADS_PER_HOUR: i64 = 60;

const MAX_FILENAME_CHARS: usize = 255;

// Request Types

/// What an uploaded file is for; it decides how the file is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilePurpose {
	/// An image for a chat message; its bytes are kept.
	Image,
	/// An image others may see too, such as a model or instance icon; its bytes are kept.
	Icon,
	/// A document for a chat message; only its extracted text is kept.
	ChatAttachment,
}

/// Query of a file upload; the body is the file itself.
#[derive(Debug, Deserialize)]
pub struct UploadFileQuery {
	/// Required for documents; images are stored without a name.
	pub filename: Option<String>,
	pub purpose: FilePurpose,
}

// Response Types

/// A stored file, without its content.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "StoredFile"))]
pub struct FileResponse {
	pub id: Uuid,
	/// `image` or `document`.
	pub kind: String,
	pub filename: Option<String>,
	pub mime_type: String,
	pub size_bytes: i64,
	/// Estimated tokens of a document's extracted text.
	pub token_count: Option<i64>,
	/// Where an image's bytes are served; documents keep no bytes.
	pub url: Option<String>,
}

// Internal Types

/// Why a file could not be stored; `reason()` is translated as `files.error.<reason>`.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
	#[error("invalid file name")]
	InvalidFilename,
	#[error("the file is too large")]
	TooLarge,
	#[error("unsupported file type")]
	UnsupportedType,
	#[error("the file is not text")]
	NotText,
	#[error("the file has no text")]
	Empty,
	#[error("the file has too much text")]
	TextTooLong,
	#[error("the file could not be read")]
	ExtractFailed,
	#[error("reading the file took too long")]
	TimedOut,
	#[error("too many files are being read right now")]
	Busy,
	#[error("the storage quota is used up")]
	QuotaExceeded,
	#[error("too many files were uploaded")]
	RateLimited,
	#[error("the file could not be saved: {0}")]
	Storage(String),
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

impl FileError {
	/// The translation suffix shown to the user.
	#[must_use]
	pub const fn reason(&self) -> &'static str {
		match self {
			Self::InvalidFilename => "invalid_filename",
			Self::TooLarge => "too_large",
			Self::UnsupportedType => "unsupported_type",
			Self::NotText => "not_text",
			Self::Empty => "empty",
			Self::TextTooLong => "text_too_long",
			Self::ExtractFailed => "extract_failed",
			Self::TimedOut => "timed_out",
			Self::Busy => "busy",
			Self::QuotaExceeded => "quota_exceeded",
			Self::RateLimited => "rate_limited",
			Self::Storage(_) | Self::Database(_) => "database",
		}
	}
}

/// The trimmed file name, or an error when it is empty, too long or holds control characters.
///
/// # Errors
///
/// Returns [`FileError::InvalidFilename`] for a name that cannot be stored.
pub fn checked_filename(filename: &str) -> Result<&str, FileError> {
	let filename = filename.trim();
	if filename.is_empty() || filename.chars().count() > MAX_FILENAME_CHARS || filename.chars().any(char::is_control) {
		return Err(FileError::InvalidFilename);
	}
	Ok(filename)
}

/// Uploads of every kind.
pub struct FileUpload;

impl FileUpload {
	/// Stores an uploaded file for the user the way its purpose asks for.
	///
	/// # Errors
	///
	/// Returns a [`FileError`] when the file is refused or cannot be stored.
	pub async fn store(pool: &PgPool, user_id: &Uuid, query: &UploadFileQuery, bytes: &[u8]) -> Result<FileResponse, FileError> {
		match query.purpose {
			FilePurpose::ChatAttachment => Ok(ChatAttachment::upload(pool, user_id, query.filename.as_deref().unwrap_or_default(), bytes).await?.to_response()),
			FilePurpose::Image => Self::store_image(pool, user_id, bytes, IMAGE_UPLOAD_SOURCE).await,
			FilePurpose::Icon => Self::store_image(pool, user_id, bytes, ICON_SOURCE).await,
		}
	}

	async fn store_image(pool: &PgPool, user_id: &Uuid, bytes: &[u8], source: &str) -> Result<FileResponse, FileError> {
		if bytes.len() > MAX_UPLOAD_BYTES {
			return Err(FileError::TooLarge);
		}
		let mime_type = detect_image_mime(bytes).ok_or(FileError::UnsupportedType)?;
		let mut tx = FileStorage::begin_store(pool, user_id, bytes.len()).await?;
		let stored = store_image_in(&mut tx, bytes, mime_type, Some(*user_id), Some(source)).await.map_err(FileError::Storage)?;
		tx.commit().await?;
		Ok(FileResponse {
			id: stored.id,
			kind: "image".to_owned(),
			filename: None,
			mime_type: stored.mime_type,
			size_bytes: stored.size_bytes,
			token_count: None,
			url: Some(image_url(stored.id)),
		})
	}
}

/// How much of the storage quota a user's files take up.
pub struct FileStorage;

impl FileStorage {
	/// Refuses an upload once the user has used up their hourly upload limit; [`Self::begin_store`] checks it again.
	///
	/// # Errors
	///
	/// Returns [`FileError::RateLimited`] or a database error.
	pub async fn check_rate(connection: &mut PgConnection, user_id: &Uuid) -> Result<(), FileError> {
		let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE user_id = $1 AND source = ANY($2) AND created_at > NOW() - INTERVAL '1 hour'")
			.bind(user_id)
			.bind([CHAT_ATTACHMENT_SOURCE, IMAGE_UPLOAD_SOURCE, ICON_SOURCE])
			.fetch_one(&mut *connection)
			.await?;
		if recent >= UPLOADS_PER_HOUR {
			return Err(FileError::RateLimited);
		}
		Ok(())
	}

	/// Starts the transaction a new file of `incoming` bytes is stored in: uploads of one user wait for each other,
	/// and the file is refused when the hourly upload limit is used up or it does not fit the storage quota an admin set.
	///
	/// # Errors
	///
	/// Returns [`FileError::RateLimited`], [`FileError::QuotaExceeded`] or a database error.
	pub async fn begin_store<'a>(pool: &'a PgPool, user_id: &Uuid, incoming: usize) -> Result<Transaction<'a, Postgres>, FileError> {
		let mut tx = pool.begin().await?;
		sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
			.bind(format!("files:{user_id}"))
			.execute(&mut *tx)
			.await?;
		Self::check_rate(&mut tx, user_id).await?;
		let storage = Config::get().storage();
		if storage.quota_mb.is_some() && !storage.allows_upload(Self::used_bytes(&mut *tx, user_id).await?, incoming) {
			return Err(FileError::QuotaExceeded);
		}
		Ok(tx)
	}

	/// Bytes the user's files take: originals that were kept, plus extracted text and captions.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn used_bytes<'e, E: PgExecutor<'e>>(executor: E, user_id: &Uuid) -> Result<i64, sqlx::Error> {
		sqlx::query_scalar(
			r#"
			SELECT COALESCE(SUM(CASE WHEN data IS NOT NULL OR file_path IS NOT NULL THEN size_bytes ELSE 0 END + COALESCE(octet_length(text), 0)), 0)::bigint
			FROM files WHERE user_id = $1
			"#,
		)
		.bind(user_id)
		.fetch_one(executor)
		.await
	}
}
