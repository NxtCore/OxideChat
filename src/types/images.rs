//! Image-related types for OxideChat API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============= Request Types =============

/// Request body for uploading an image
#[derive(Debug, Deserialize)]
pub struct UploadImageRequest {
	/// Base64 data URI (e.g., "data:image/png;base64,...")
	pub data_uri: String,
	/// Optional user ID for attribution
	pub user_id: Option<Uuid>,
	/// Optional source identifier (e.g., "imagegen")
	pub source: Option<String>,
}

// ============= Response Types =============

/// Response for successful image upload
#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
	pub id: Uuid,
	pub url: String,
	pub mime_type: String,
	pub size_bytes: i64,
}
