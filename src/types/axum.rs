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

/// Request body for `PATCH /api/v1/admin/models/:id`.
///
/// Every field is optional so callers only need to send what they want to change.
/// For fields that can hold `null` in the database, the outer `Option` controls
/// whether the field is included in the request at all, and the inner `Option`
/// is the actual value:
///
/// - field absent from JSON  → `None`        → column is not touched
/// - `"field": null` in JSON → `Some(None)`  → column is set to NULL
/// - `"field": <v>` in JSON  → `Some(Some(v))` → column is set to `v`
///
/// Non-nullable fields (`display_name`, `is_enabled`, etc.) use a plain
/// `Option<T>` because they can never be NULL in the database.
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
