use super::{PresetMode, MAX_PRESET_LEVELS, MAX_PRESET_SLUG_LEN, MAX_PRESET_STARTERS, MIN_PRESET_LEVELS, PresetLevel, PresetReasoning, PresetSampling, PresetScope, SavePresetRequest};

const MAX_NAME_LEN: usize = 100;
const MAX_DESCRIPTION_LEN: usize = 2_000;
const MAX_ICON_LEN: usize = 500;
const MAX_SYSTEM_PROMPT_LEN: usize = 50_000;
const MAX_STARTER_LEN: usize = 300;
const MAX_STOP_SEQUENCES: usize = 4;
const MAX_TOOLS: usize = 64;
const MAX_LEVEL_KEY_LEN: usize = 32;
const MAX_LEVEL_LABEL_LEN: usize = 40;
const MAX_LEVEL_HINT_LEN: usize = 100;
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
	#[error("an arena needs 2 to 5 different models, no tools and a team or installation scope")]
	ArenaInvalid,
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

/// Whether `key` can name a level: lowercase letters, digits and inner hyphens, at most 32 characters.
#[must_use]
pub fn is_valid_level_key(key: &str) -> bool {
	key.len() <= MAX_LEVEL_KEY_LEN && is_valid_slug_part(key)
}

impl PresetLevel {
	fn normalized(mut self) -> Result<Self, PresetError> {
		self.key = self.key.trim().to_string();
		self.label = optional_text(self.label, MAX_LEVEL_LABEL_LEN)?;
		self.hint = optional_text(self.hint, MAX_LEVEL_HINT_LEN)?;
		if is_valid_level_key(&self.key) && self.reasoning.is_valid() { Ok(self) } else { Err(PresetError::Invalid) }
	}
}

fn normalized_levels(levels: Vec<PresetLevel>, default_level: Option<String>) -> Result<(Vec<PresetLevel>, Option<String>), PresetError> {
	if levels.is_empty() {
		return Ok((levels, None));
	}
	if !(MIN_PRESET_LEVELS..=MAX_PRESET_LEVELS).contains(&levels.len()) {
		return Err(PresetError::Invalid);
	}
	let levels = levels.into_iter().map(PresetLevel::normalized).collect::<Result<Vec<_>, _>>()?;
	let mut keys: Vec<&str> = levels.iter().map(|level| level.key.as_str()).collect();
	keys.sort_unstable();
	keys.dedup();
	if keys.len() != levels.len() {
		return Err(PresetError::Invalid);
	}
	let default_level = match default_level.map(|key| key.trim().to_string()) {
		Some(key) if levels.iter().any(|level| level.key == key) => key,
		Some(_) => return Err(PresetError::Invalid),
		None => levels[0].key.clone(),
	};
	Ok((levels, Some(default_level)))
}

impl SavePresetRequest {
	fn check_arena(&self) -> Result<(), PresetError> {
		let mut models: Vec<_> = self.levels.iter().map(|level| level.model_id).collect();
		models.sort_unstable();
		models.dedup();
		let personal = self.scope.is_none_or(|scope| scope == PresetScope::Personal);
		if self.levels.is_empty() || models.len() != self.levels.len() || !self.tool_ids.is_empty() || personal {
			return Err(PresetError::ArenaInvalid);
		}
		Ok(())
	}

	/// Trims the text fields and checks every limit, returning the preset as it will be stored.
	///
	/// A preset with levels needs 2 to 5 of them with unique keys; its default level is the first one unless named,
	/// and its base model becomes the default level's model.
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
		(self.levels, self.default_level) = normalized_levels(self.levels, self.default_level)?;
		if self.mode == Some(PresetMode::Arena) {
			self.check_arena()?;
		}
		if let Some(level) = self.default_level.as_ref().and_then(|key| self.levels.iter().find(|level| &level.key == key)) {
			self.base_model_id = level.model_id;
		}
		Ok(self)
	}
}
