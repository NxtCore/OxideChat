use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct PaginatedResponse<T> {
	pub has_more: bool,
	pub items: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct ModelListParams {
	pub page: Option<i64>,
	pub size: Option<i64>,
	pub query: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminModelUpdateBody {
	pub display_name: Option<String>,
	pub is_enabled: Option<bool>,
	pub system_prompt: Option<String>,
	pub sampling: Option<Value>,
	pub icon: Option<String>,
	pub description: Option<String>,
	pub input_modalities: Option<Value>,
	pub output_modalities: Option<Value>,
	pub context_length: Option<i32>,
	pub max_output_tokens: Option<i32>,
	pub enabled_tools: Option<Value>,
	pub is_public: Option<bool>,
	pub is_featured: Option<bool>,
	pub is_default: Option<bool>,
	pub is_favorite: Option<bool>,
	pub category: Option<String>,
	pub tags: Option<Value>,
	pub extra_settings: Option<Value>,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<u32>,
}
