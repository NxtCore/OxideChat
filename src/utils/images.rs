//! Image Storage Module
//!
//! Provides configurable storage backends for images (database or filesystem).
//! Set `IMAGE_STORAGE_TYPE` env var to "database" or "file" (default: "database").
//! For file storage, set `IMAGE_STORAGE_PATH` (default: "./uploads/images").

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sqlx::PgPool;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// Storage type for images
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
	Database,
	File,
}

impl StorageType {
	pub fn from_env() -> Self {
		match std::env::var("IMAGE_STORAGE_TYPE").as_deref() {
			Ok("file") | Ok("filesystem") => StorageType::File,
			_ => StorageType::Database,
		}
	}
}

/// Get the base path for file storage
pub fn storage_path() -> PathBuf {
	std::env::var("IMAGE_STORAGE_PATH")
		.map(PathBuf::from)
		.unwrap_or_else(|_| PathBuf::from("./uploads/images"))
}

/// Generate a URL path for an image (relative, works behind any reverse proxy)
pub fn image_url(id: Uuid) -> String {
	format!("/api/v1/images/{}", id)
}
/// Stored image metadata
#[derive(Debug, Clone)]
pub struct StoredImage {
	pub id: Uuid,
	pub mime_type: String,
	pub size_bytes: i64,
}

/// Store an image from base64 data
///
/// Returns the stored image metadata including the generated UUID.
pub async fn store_image(db: &PgPool, data: &[u8], mime_type: &str, user_id: Option<Uuid>, source: Option<&str>) -> Result<StoredImage, String> {
	let storage_type = StorageType::from_env();
	let id = Uuid::new_v4();
	let size_bytes = i64::try_from(data.len()).map_err(|_| "Image size too large to represent in 64-bit".to_string())?;

	match storage_type {
		StorageType::Database => {
			sqlx::query(
				r#"
				INSERT INTO images (id, data, mime_type, size_bytes, user_id, source)
				VALUES ($1, $2, $3, $4, $5, $6)
				"#,
			)
			.bind(id)
			.bind(data)
			.bind(mime_type)
			.bind(size_bytes)
			.bind(user_id)
			.bind(source)
			.execute(db)
			.await
			.map_err(|e| format!("Failed to store image in database: {e}"))?;
		}
		StorageType::File => {
			let base_path = storage_path();
			fs::create_dir_all(&base_path).await.map_err(|e| format!("Failed to create storage directory: {e}"))?;

			let extension = mime_to_extension(mime_type);
			let filename = format!("{id}.{extension}");
			let file_path = base_path.join(&filename);

			fs::write(&file_path, data).await.map_err(|e| format!("Failed to write image file: {e}"))?;

			let relative_path = filename;

			let db_result = sqlx::query(
				r#"
				INSERT INTO images (id, file_path, mime_type, size_bytes, user_id, source)
				VALUES ($1, $2, $3, $4, $5, $6)
				"#,
			)
			.bind(id)
			.bind(&relative_path)
			.bind(mime_type)
			.bind(size_bytes)
			.bind(user_id)
			.bind(source)
			.execute(db)
			.await;

			if let Err(e) = db_result {
				let _ = fs::remove_file(&file_path).await;
				return Err(format!("Failed to store image metadata: {e}"));
			}
		}
	}

	Ok(StoredImage {
		id,
		mime_type: mime_type.to_string(),
		size_bytes,
	})
}

/// Store an image from a base64 data URI (e.g., "data:image/png;base64,...")
pub async fn store_from_data_uri(db: &PgPool, data_uri: &str, user_id: Option<Uuid>, source: Option<&str>) -> Result<StoredImage, String> {
	let (mime_type, data) = parse_data_uri(data_uri)?;
	store_image(db, &data, &mime_type, user_id, source).await
}

#[must_use]
pub fn safe_image_mime(data: &[u8], declared_mime: &str) -> Option<&'static str> {
	let detected = infer::get(data)?.mime_type();
	let declared = normalize_image_mime(declared_mime)?;

	if detected == declared { Some(declared) } else { None }
}

/// Retrieve an image by ID
///
/// Returns (data, mime_type) or None if not found.
pub async fn get_image(db: &PgPool, id: Uuid) -> Result<Option<(Vec<u8>, String)>, String> {
	#[derive(sqlx::FromRow)]
	struct ImageRow {
		data: Option<Vec<u8>>,
		file_path: Option<String>,
		mime_type: String,
	}

	let row: Option<ImageRow> = sqlx::query_as(
		r#"
		SELECT data, file_path, mime_type FROM images WHERE id = $1
		"#,
	)
	.bind(id)
	.fetch_optional(db)
	.await
	.map_err(|e| format!("Failed to fetch image: {e}"))?;

	match row {
		Some(img) => {
			let data = if let Some(data) = img.data {
				data
			} else if let Some(file_path) = img.file_path {
				let full_path = storage_path().join(&file_path);
				fs::read(&full_path).await.map_err(|e| format!("Failed to read image file: {e}"))?
			} else {
				return Err("Image data not found in database or file".to_string());
			};
			Ok(Some((data, img.mime_type)))
		}
		None => Ok(None),
	}
}

/// Store a caption (typically the generation prompt) for an image.
///
/// Used as a text stand-in when rehydrating history for non-vision models.
pub async fn set_image_caption(db: &PgPool, id: Uuid, caption: &str) -> Result<(), String> {
	sqlx::query("UPDATE images SET caption = $2 WHERE id = $1")
		.bind(id)
		.bind(caption)
		.execute(db)
		.await
		.map_err(|e| format!("Failed to set image caption: {e}"))?;
	Ok(())
}

/// Retrieve the stored caption for an image, if any.
pub async fn image_caption(db: &PgPool, id: Uuid) -> Result<Option<String>, String> {
	let row: Option<(Option<String>,)> = sqlx::query_as("SELECT caption FROM images WHERE id = $1")
		.bind(id)
		.fetch_optional(db)
		.await
		.map_err(|e| format!("Failed to fetch image caption: {e}"))?;
	Ok(row.and_then(|r| r.0))
}

/// Parse a base64 data URI into (mime_type, decoded_bytes)
pub(crate) fn parse_data_uri(data_uri: &str) -> Result<(String, Vec<u8>), String> {
	if !data_uri.starts_with("data:") {
		return Err("Invalid data URI: must start with 'data:'".to_string());
	}

	let without_prefix = &data_uri[5..];
	let Some((header, data)) = without_prefix.split_once(',') else {
		return Err("Invalid data URI format".to_string());
	};

	if !header.split(';').skip(1).any(|part| part.eq_ignore_ascii_case("base64")) {
		return Err("Invalid data URI: image data must be base64 encoded".to_string());
	}

	let declared_mime = if header.contains(';') {
		header.split(';').next().unwrap_or_default()
	} else {
		header
	};
	let Some(mime_type) = normalize_image_mime(declared_mime) else {
		return Err("Unsupported image type".to_string());
	};

	let decoded = BASE64.decode(data).map_err(|e| format!("Failed to decode base64: {e}"))?;
	if safe_image_mime(&decoded, mime_type).is_none() {
		return Err("Image content does not match a supported raster image type".to_string());
	}

	Ok((mime_type.to_string(), decoded))
}

fn normalize_image_mime(mime: &str) -> Option<&'static str> {
	match mime.trim().to_ascii_lowercase().as_str() {
		"image/png" => Some("image/png"),
		"image/jpeg" | "image/jpg" => Some("image/jpeg"),
		"image/gif" => Some("image/gif"),
		"image/webp" => Some("image/webp"),
		_ => None,
	}
}

/// Convert MIME type to file extension
fn mime_to_extension(mime: &str) -> &'static str {
	match mime {
		"image/png" => "png",
		"image/jpeg" | "image/jpg" => "jpg",
		"image/gif" => "gif",
		"image/webp" => "webp",
		_ => "bin",
	}
}

/// Check if an image URL is a base64 data URI that needs to be uploaded
pub fn is_data_uri(url: &str) -> bool {
	url.starts_with("data:")
}
