use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UploadImageRequest {
	pub data_uri: String,
	pub source: Option<String>,
}
