use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpsertTranslationRequest {
	pub language: String,
	pub key_path: String,
	pub value: String,
	#[serde(default)]
	pub is_override: bool,
}
