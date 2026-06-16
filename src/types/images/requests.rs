use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UploadImageRequest {
	pub data_uri: String,
	pub user_id: Option<Uuid>,
	pub source: Option<String>,
}
