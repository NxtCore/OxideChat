use crate::types::{AssignmentKind, AssignmentTarget};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

mod admin;
mod gateway;
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

/// Separates a preset key from the level it asks for, as in `presets/gemini:high`.
pub const PRESET_LEVEL_SEPARATOR: char = ':';

/// Fewest levels a preset with levels must have.
pub const MIN_PRESET_LEVELS: usize = 2;

/// Most levels a preset can have.
pub const MAX_PRESET_LEVELS: usize = 5;

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

/// One step of a preset with levels: the model it runs on and, optionally, the reasoning it uses there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetLevel {
	/// Short ID of the level, used in `presets/<slug>:<key>`; `low`, `medium`, `high` and `max` have translated labels.
	pub key: String,
	/// Custom label; without one the translated label of the key is shown.
	#[serde(default)]
	pub label: Option<String>,
	/// Short hint shown with the level, such as "Hard problems".
	#[serde(default)]
	pub hint: Option<String>,
	pub model_id: Uuid,
	/// Reasoning this level uses; when empty the preset's reasoning applies.
	#[serde(default)]
	pub reasoning: PresetReasoning,
}

/// A row of `model_presets`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Preset {
	pub id: Uuid,
	pub scope: String,
	pub owner_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub team_name: Option<String>,
	pub slug: String,
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub base_model_id: Option<Uuid>,
	pub system_prompt: Option<String>,
	pub sampling: Json<PresetSampling>,
	pub reasoning: Json<PresetReasoning>,
	pub levels: Json<Vec<PresetLevel>>,
	pub default_level: Option<String>,
	pub tool_ids: Vec<Uuid>,
	pub tools_locked: bool,
	pub starters: Vec<String>,
	pub prompt_visible: bool,
	pub is_enabled: bool,
	pub disabled_by_admin: bool,
	pub updated_at: DateTime<Utc>,
}

impl Preset {
	/// The model key clients send for this preset, `presets/<slug>`.
	#[must_use]
	pub fn key(&self) -> String {
		format!("{PRESET_KEY_PREFIX}{}", self.slug)
	}

	/// The scope stored on the row.
	#[must_use]
	pub fn scope(&self) -> PresetScope {
		PresetScope::from_stored(&self.scope)
	}

	/// The level `key` names, or the default level when `key` is `None`; `None` for a preset without levels.
	#[must_use]
	pub fn level(&self, key: Option<&str>) -> Option<&PresetLevel> {
		let key = key.or(self.default_level.as_deref())?;
		self.levels.iter().find(|level| level.key == key)
	}

	/// Every model the preset runs on: its base model and the models of its levels.
	pub fn model_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
		self.base_model_id.into_iter().chain(self.levels.iter().map(|level| level.model_id))
	}

	/// The levels a key may run on, in the order to try them: the named level, or for a bare key the default level and then the others.
	#[must_use]
	pub fn level_candidates(&self, key: Option<&str>) -> Vec<Option<String>> {
		if key.is_some() || self.levels.is_empty() {
			return vec![key.map(str::to_string)];
		}
		let default = self.default_level.as_deref();
		let others = self.levels.iter().map(|level| level.key.as_str()).filter(|level| Some(*level) != default);
		default.into_iter().chain(others).map(|level| Some(level.to_string())).collect()
	}

	/// The model the preset runs on at level `key`, resolved like [`Preset::at_level`].
	#[must_use]
	pub fn model_at(&self, key: Option<&str>) -> Option<Uuid> {
		if self.levels.is_empty() {
			return self.base_model_id.filter(|_| key.is_none());
		}
		self.level(key).map(|level| level.model_id)
	}

	/// The preset as it runs at level `key`: its base model and reasoning become the level's.
	///
	/// Without levels only `None` resolves; with levels `None` picks the default level. Returns `None` for an unknown level.
	#[must_use]
	pub fn at_level(mut self, key: Option<&str>) -> Option<Self> {
		if self.levels.is_empty() {
			return key.is_none().then_some(self);
		}
		let level = self.level(key)?.clone();
		self.base_model_id = Some(level.model_id);
		if level.reasoning.effort.is_some() || level.reasoning.budget_tokens.is_some() {
			self.reasoning = Json(level.reasoning);
		}
		Some(self)
	}
}

/// Who is acting on presets and which scopes they may manage.
#[derive(Debug, Clone)]
pub struct PresetActor {
	pub user_id: Uuid,
	pub personal: bool,
	pub installation_admin: bool,
	/// Whether the actor may view users, which picking a single user to share with needs.
	pub view_users: bool,
	pub admin_teams: Vec<Uuid>,
	pub teams: Vec<Uuid>,
	pub editable: Vec<Uuid>,
}

impl PresetActor {
	/// Whether the actor may create or change presets of `scope`, for team presets in `team_id`.
	#[must_use]
	pub fn can_manage_scope(&self, scope: PresetScope, team_id: Option<&Uuid>) -> bool {
		match scope {
			PresetScope::Personal => self.personal,
			PresetScope::Team => self.installation_admin || team_id.is_some_and(|team| self.admin_teams.contains(team)),
			PresetScope::Installation => self.installation_admin,
		}
	}

	/// Whether `preset` lives in the actor's own scope: their personal preset, one of their teams' or the installation's.
	#[must_use]
	pub fn is_home(&self, preset: &Preset) -> bool {
		match preset.scope() {
			PresetScope::Personal => preset.owner_id == Some(self.user_id),
			PresetScope::Team => preset.team_id.is_some_and(|team| self.teams.contains(&team)),
			PresetScope::Installation => true,
		}
	}

	/// Whether the actor may share presets with `kind`, for teams with `team_id`: teams they manage, and for installation
	/// admins also everyone and, when they may view users, single users.
	#[must_use]
	pub fn can_share_with(&self, kind: AssignmentKind, team_id: Option<&Uuid>) -> bool {
		match kind {
			AssignmentKind::Team => self.can_manage_scope(PresetScope::Team, team_id),
			AssignmentKind::User => self.installation_admin && self.view_users,
			AssignmentKind::Everyone => self.installation_admin,
			AssignmentKind::Role | AssignmentKind::Key => false,
		}
	}

	/// Whether the actor may change `preset`: its managers and everyone it is shared with as an editor.
	#[must_use]
	pub fn can_edit(&self, preset: &Preset) -> bool {
		self.can_manage(preset) || self.editable.contains(&preset.id)
	}

	/// Whether a share reaches the actor: a share with them, or with one of their teams.
	#[must_use]
	pub fn is_share_target(&self, target: &AssignmentTarget) -> bool {
		match (target.kind, target.id) {
			(AssignmentKind::User, Some(id)) => id == self.user_id,
			(AssignmentKind::Team, Some(id)) => self.teams.contains(&id),
			_ => false,
		}
	}

	/// Whether the actor owns `preset` and so may delete it and change who it is shared with.
	#[must_use]
	pub fn can_manage(&self, preset: &Preset) -> bool {
		match preset.scope() {
			PresetScope::Personal => self.personal && preset.owner_id == Some(self.user_id),
			scope => self.can_manage_scope(scope, preset.team_id.as_ref()),
		}
	}
}

/// The slug of a preset key, or `None` when the key is not in the preset namespace.
#[must_use]
pub fn preset_slug(key: &str) -> Option<&str> {
	key.strip_prefix(PRESET_KEY_PREFIX).filter(|slug| !slug.is_empty())
}

/// The slug and the level of a preset key such as `presets/gemini:high`, or `None` when the key is not in the preset namespace.
#[must_use]
pub fn preset_key_parts(key: &str) -> Option<(&str, Option<&str>)> {
	let rest = preset_slug(key)?;
	match rest.split_once(PRESET_LEVEL_SEPARATOR) {
		Some((slug, level)) if !slug.is_empty() && !level.is_empty() => Some((slug, Some(level))),
		Some(_) => None,
		None => Some((rest, None)),
	}
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
