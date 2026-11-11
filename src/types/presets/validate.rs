use super::{MAX_PRESET_SLUG_LEN, MAX_PRESET_STARTERS, PresetReasoning, PresetSampling, PresetScope, SavePresetRequest};

const MAX_NAME_LEN: usize = 100;
const MAX_DESCRIPTION_LEN: usize = 2_000;
const MAX_ICON_LEN: usize = 500;
const MAX_SYSTEM_PROMPT_LEN: usize = 50_000;
const MAX_STARTER_LEN: usize = 300;
const MAX_STOP_SEQUENCES: usize = 4;
const MAX_TOOLS: usize = 64;
const REASONING_EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh"];

/// Why a preset could not be saved or used.
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
	#[error("the slug is not a valid preset ID")]
	InvalidSlug,
	#[error("the preset ID is already in use")]
	SlugTaken,
	#[error("the preset has invalid fields")]
	Invalid,
	#[error("the base model does not exist or may not be used")]
	BaseModelNotAllowed,
	#[error("a tool does not exist or may not be used")]
	ToolsNotAllowed,
	#[error("the preset does not exist")]
	NotFound,
	#[error("the preset may not be managed by this user")]
	Forbidden,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

fn is_valid_slug_part(part: &str) -> bool {
	let bytes = part.as_bytes();
	let edge = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
	bytes.first().is_some_and(edge) && bytes.last().is_some_and(edge) && bytes.iter().all(|b| edge(b) || *b == b'-')
}

/// Whether `slug` can be part of a preset ID: parts of lowercase letters, digits and inner hyphens,
/// separated by slashes, such as `team/code-reviewer`.
#[must_use]
pub fn is_valid_preset_slug(slug: &str) -> bool {
	!slug.is_empty() && slug.len() <= MAX_PRESET_SLUG_LEN && slug.split('/').all(is_valid_slug_part)
}

fn in_range(value: Option<f32>, min: f32, max: f32) -> bool {
	value.is_none_or(|v| v.is_finite() && (min..=max).contains(&v))
}

fn optional_text(value: Option<String>, max: usize) -> Result<Option<String>, PresetError> {
	let value = value.map(|text| text.trim().to_string()).filter(|text| !text.is_empty());
	if value.as_ref().is_some_and(|text| text.chars().count() > max) {
		return Err(PresetError::Invalid);
	}
	Ok(value)
}

impl PresetSampling {
	/// Returns `true` when every value is in the range providers accept.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		in_range(self.temperature, 0.0, 2.0)
			&& in_range(self.top_p, 0.0, 1.0)
			&& in_range(self.frequency_penalty, -2.0, 2.0)
			&& in_range(self.presence_penalty, -2.0, 2.0)
			&& self.max_tokens.is_none_or(|tokens| tokens > 0)
			&& self.top_k.is_none_or(|k| k > 0)
			&& self.top_logprobs.is_none_or(|n| n <= 20)
			&& self.stop.as_ref().is_none_or(|stop| stop.len() <= MAX_STOP_SEQUENCES && stop.iter().all(|s| !s.is_empty()))
	}
}

impl PresetReasoning {
	/// Returns `true` when the effort is a known level and the budget is positive.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		self.effort.as_deref().is_none_or(|effort| REASONING_EFFORTS.contains(&effort)) && self.budget_tokens.is_none_or(|tokens| tokens > 0)
	}
}

impl SavePresetRequest {
	/// Trims the text fields and checks every limit, returning the preset as it will be stored.
	///
	/// # Errors
	/// Returns [`PresetError::InvalidSlug`] for a malformed slug and [`PresetError::Invalid`] for any other field.
	pub fn normalized(mut self) -> Result<Self, PresetError> {
		match self.scope {
			Some(PresetScope::Team) if self.team_id.is_none() => return Err(PresetError::Invalid),
			Some(PresetScope::Team) => {}
			_ => self.team_id = None,
		}
		self.slug = self.slug.trim().to_string();
		if !is_valid_preset_slug(&self.slug) {
			return Err(PresetError::InvalidSlug);
		}
		self.name = self.name.trim().to_string();
		if self.name.is_empty() || self.name.chars().count() > MAX_NAME_LEN {
			return Err(PresetError::Invalid);
		}
		self.description = optional_text(self.description, MAX_DESCRIPTION_LEN)?;
		self.icon = optional_text(self.icon, MAX_ICON_LEN)?;
		self.system_prompt = optional_text(self.system_prompt, MAX_SYSTEM_PROMPT_LEN)?;
		self.starters = self.starters.into_iter().map(|starter| starter.trim().to_string()).filter(|starter| !starter.is_empty()).collect();
		if self.starters.len() > MAX_PRESET_STARTERS || self.starters.iter().any(|starter| starter.chars().count() > MAX_STARTER_LEN) {
			return Err(PresetError::Invalid);
		}
		self.tool_ids.sort_unstable();
		self.tool_ids.dedup();
		if self.tool_ids.len() > MAX_TOOLS || !self.sampling.is_valid() || !self.reasoning.is_valid() {
			return Err(PresetError::Invalid);
		}
		Ok(self)
	}
}
