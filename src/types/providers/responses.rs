use super::{Provider, ProviderKind};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ProviderResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub has_api_key: bool,
	pub extra_headers: Value,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProviderModelResponse {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct TestProviderResponse {
	pub success: bool,
	pub models_found: usize,
	pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SyncProviderResponse {
	pub success: bool,
	pub models_added: usize,
	pub models_updated: usize,
	pub models_removed: usize,
	pub message: String,
}

impl From<Provider> for ProviderResponse {
	fn from(provider: Provider) -> Self {
		Self {
			id: provider.id,
			owner_id: provider.owner_id,
			kind: provider.kind,
			name: provider.name,
			base_url: provider.base_url,
			has_api_key: provider.api_key.is_some(),
			extra_headers: provider.extra_headers.0,
			is_enabled: provider.is_enabled,
			created_at: provider.created_at,
			updated_at: provider.updated_at,
		}
	}
}
