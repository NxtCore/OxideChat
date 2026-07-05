use crate::types::providers::ProviderSlim;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProviderTab {
	pub id: Uuid,
	pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ModelListPublic {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider: ProviderSlim,
	pub icon: Option<String>,
	pub is_favorite: bool,
	pub budget_blocked: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelDetailed {
	pub id: Uuid,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider: ProviderSlim,
	pub icon: Option<String>,
	pub description: Option<String>,
	pub system_prompt: Option<String>,
	pub sampling: Option<Value>,
	pub extra_settings: Option<Value>,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub is_favorite: bool,
	pub category: Option<String>,
	pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelListAdmin {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub provider: ProviderSlim,
	pub icon: Option<String>,
}
