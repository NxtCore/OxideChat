use super::{PresetReasoning, PresetSampling};
use serde::Deserialize;
use uuid::Uuid;

/// Body for creating or replacing a preset; a save always sends the whole preset.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SavePresetRequest {
	pub slug: String,
	pub name: String,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub icon: Option<String>,
	pub base_model_id: Uuid,
	#[serde(default)]
	pub system_prompt: Option<String>,
	#[serde(default)]
	pub sampling: PresetSampling,
	#[serde(default)]
	pub reasoning: PresetReasoning,
	#[serde(default)]
	pub tool_ids: Vec<Uuid>,
	#[serde(default)]
	pub tools_locked: bool,
	#[serde(default)]
	pub starters: Vec<String>,
	#[serde(default = "default_true")]
	pub prompt_visible: bool,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

/// Query for checking whether a slug is still free.
#[derive(Debug, Deserialize)]
pub struct PresetSlugQuery {
	pub slug: String,
	pub exclude_id: Option<Uuid>,
}

const fn default_true() -> bool {
	true
}
