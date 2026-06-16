use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
	pub id: Uuid,
	pub url: String,
	pub mime_type: String,
	pub size_bytes: i64,
}
