use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

mod repository;
mod requests;
mod responses;
mod rows;
mod validate;

pub use requests::*;
pub use responses::*;
pub use validate::*;

/// Every preset is addressed as `presets/<slug>`; no provider or model may use this namespace.
pub const PRESET_KEY_PREFIX: &str = "presets/";

/// Provider name that would put a provider's models into the preset namespace.
pub const RESERVED_PROVIDER_NAME: &str = "presets";

/// Request metadata key that carries the preset a request was made with.
pub const PRESET_METADATA_KEY: &str = "oxide_preset_id";

/// Longest allowed slug, matching the `model_presets.slug` column.
pub const MAX_PRESET_SLUG_LEN: usize = 64;

/// Most conversation starters a preset can show.
pub const MAX_PRESET_STARTERS: usize = 4;

/// Who a preset belongs to and who can see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum PresetScope {
	Personal,
	Team,
	Installation,
}

impl PresetScope {
	/// The value stored in `model_presets.scope`.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Personal => "personal",
			Self::Team => "team",
			Self::Installation => "installation",
		}
	}

	/// Parses the stored scope; unknown values are treated as personal.
	#[must_use]
	pub fn from_stored(value: &str) -> Self {
		match value {
			"team" => Self::Team,
			"installation" => Self::Installation,
			_ => Self::Personal,
		}
	}
}

/// Sampling defaults a preset applies; any field left empty falls through to the model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetSampling {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_p: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_k: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub frequency_penalty: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub presence_penalty: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub seed: Option<i64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub stop: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub logprobs: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_logprobs: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub parallel_tool_calls: Option<bool>,
}

impl PresetSampling {
	/// Converts the preset's values into Omniference sampling options.
	#[must_use]
	pub fn to_sampling(&self) -> omniference::Sampling {
		omniference::Sampling {
			temperature: self.temperature,
			top_p: self.top_p,
			top_k: self.top_k,
			max_tokens: self.max_tokens,
			presence_penalty: self.presence_penalty,
			frequency_penalty: self.frequency_penalty,
			stop: self.stop.clone().unwrap_or_default(),
			parallel_tool_calls: self.parallel_tool_calls,
			seed: self.seed,
			logit_bias: None,
			logprobs: self.logprobs,
			top_logprobs: self.top_logprobs,
		}
	}
}

/// Reasoning defaults a preset applies when the request sets none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetReasoning {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub effort: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub budget_tokens: Option<u32>,
}

/// A row of `model_presets`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Preset {
	pub id: Uuid,
	pub scope: String,
	pub owner_id: Option<Uuid>,
	pub slug: String,
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub base_model_id: Option<Uuid>,
	pub system_prompt: Option<String>,
	pub sampling: Json<PresetSampling>,
	pub reasoning: Json<PresetReasoning>,
	pub tool_ids: Vec<Uuid>,
	pub tools_locked: bool,
	pub starters: Vec<String>,
	pub prompt_visible: bool,
	pub is_enabled: bool,
	pub updated_at: DateTime<Utc>,
}

impl Preset {
	/// The model key clients send for this preset, `presets/<slug>`.
	#[must_use]
	pub fn key(&self) -> String {
		format!("{PRESET_KEY_PREFIX}{}", self.slug)
	}

	/// Whether `user_id` may edit this preset.
	#[must_use]
	pub fn is_owned_by(&self, user_id: &Uuid) -> bool {
		self.owner_id.as_ref() == Some(user_id)
	}
}

/// The slug of a preset key, or `None` when the key is not in the preset namespace.
#[must_use]
pub fn preset_slug(key: &str) -> Option<&str> {
	key.strip_prefix(PRESET_KEY_PREFIX).filter(|slug| !slug.is_empty())
}

/// Whether a model ID falls into the reserved preset namespace.
#[must_use]
pub fn is_reserved_model_id(model_id: &str) -> bool {
	model_id.trim().to_ascii_lowercase().starts_with(PRESET_KEY_PREFIX)
}

/// Whether a provider name would put its models into the reserved preset namespace.
#[must_use]
pub fn is_reserved_provider_name(name: &str) -> bool {
	name.trim().eq_ignore_ascii_case(RESERVED_PROVIDER_NAME)
}
