use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ModelListParams {
	pub page: Option<i64>,
	pub size: Option<i64>,
	pub query: Option<String>,
	pub is_favorite: Option<bool>,
	pub provider_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AdminModelPatchBody {
	pub display_name: Option<String>,
	pub is_enabled: Option<bool>,
	pub system_prompt: Option<Option<String>>,
	pub sampling: Option<Option<Value>>,
	pub icon: Option<Option<String>>,
	pub description: Option<Option<String>>,
	pub input_modalities: Option<Option<Value>>,
	pub output_modalities: Option<Option<Value>>,
	pub context_length: Option<Option<i32>>,
	pub max_output_tokens: Option<Option<i32>>,
	pub enabled_tools: Option<Option<Value>>,
	pub is_public: Option<bool>,
	pub is_featured: Option<bool>,
	pub is_default: Option<bool>,
	pub is_favorite: Option<bool>,
	pub category: Option<Option<String>>,
	pub tags: Option<Option<Value>>,
	pub extra_settings: Option<Option<Value>>,
	pub reasoning_effort: Option<Option<String>>,
	pub reasoning_budget_tokens: Option<Option<u32>>,
}

#[derive(Debug, Deserialize)]
pub struct ModelPricingOverrideRequest {
	pub pricing: omniference::catalog::ModelPricing,
}
