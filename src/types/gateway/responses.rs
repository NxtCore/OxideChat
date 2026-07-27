use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OpenAiErrorResponse {
	pub error: OpenAiError,
}

#[derive(Debug, Serialize)]
pub struct OpenAiError {
	pub message: String,
	#[serde(rename = "type")]
	pub kind: String,
	pub param: Option<String>,
	pub code: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OpenAiModel {
	pub id: String,
	pub object: String,
	pub created: i64,
	pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAiModelsResponse {
	pub object: String,
	pub data: Vec<OpenAiModel>,
}
