use super::{Preset, PresetActor, PresetArena, PresetLevel, PresetMode, PresetReasoning, PresetSampling, PresetScope};
use crate::types::models::ModelListPublic;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Why a preset cannot be used right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PresetUnavailable {
	Disabled,
	BaseModelUnavailable,
	BaseModelNotAllowed,
}

/// A preset as its viewer sees it, with its base model as the model picker lists it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetResponse {
	pub id: Uuid,
	pub key: String,
	pub slug: String,
	pub scope: PresetScope,
	pub team: Option<PresetTeam>,
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub base_model_id: Option<Uuid>,
	pub base_model: Option<ModelListPublic>,
	pub system_prompt: Option<String>,
	pub sampling: PresetSampling,
	pub reasoning: PresetReasoning,
	/// The levels users pick between, in order; empty for a preset without levels.
	pub levels: Vec<PresetLevelResponse>,
	pub default_level: Option<String>,
	pub tool_ids: Vec<Uuid>,
	pub tools_locked: bool,
	pub starters: Vec<String>,
	pub prompt_visible: bool,
	pub is_enabled: bool,
	pub disabled_by_admin: bool,
	pub mode: PresetMode,
	pub arena: PresetArena,
	pub has_gateway_usage: bool,
	pub can_edit: bool,
	/// Whether the viewer may delete the preset and change who it is shared with.
	pub can_manage: bool,
	/// Whether the viewer only sees the preset because it was shared with them.
	pub shared: bool,
	pub unavailable: Option<PresetUnavailable>,
	pub updated_at: DateTime<Utc>,
}

/// One level of a preset with its model as the viewer sees it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetLevelResponse {
	pub key: String,
	pub label: Option<String>,
	pub hint: Option<String>,
	pub model_id: Uuid,
	pub reasoning: PresetReasoning,
	pub model: Option<ModelListPublic>,
	pub unavailable: Option<PresetUnavailable>,
}

/// A team a preset belongs to or can be shared with.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetTeam {
	pub id: Uuid,
	pub name: String,
}

/// The scopes a user may create presets in.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetScopesResponse {
	pub personal: bool,
	pub teams: Vec<PresetTeam>,
	pub installation: bool,
}

/// The user a personal preset belongs to, as admins see it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetOwner {
	pub id: Uuid,
	pub username: String,
}

/// A preset in the installation-wide admin list.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AdminPresetResponse {
	pub id: Uuid,
	pub key: String,
	pub slug: String,
	pub scope: PresetScope,
	pub name: String,
	pub icon: Option<String>,
	pub team: Option<PresetTeam>,
	pub owner: Option<PresetOwner>,
	pub base_model_name: Option<String>,
	pub is_enabled: bool,
	pub updated_at: DateTime<Utc>,
}

/// Whether a slug can be used for a new or renamed preset.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PresetSlugAvailability {
	pub valid: bool,
	pub available: bool,
}

/// The base model of a preset as one viewer may see it.
pub struct PresetBaseModel {
	pub model: ModelListPublic,
	pub allowed: bool,
}

fn model_unavailable(model: Option<&PresetBaseModel>) -> Option<PresetUnavailable> {
	match model {
		None => Some(PresetUnavailable::BaseModelUnavailable),
		Some(model) if !model.allowed => Some(PresetUnavailable::BaseModelNotAllowed),
		Some(_) => None,
	}
}

impl PresetLevelResponse {
	fn build(level: PresetLevel, model: Option<PresetBaseModel>) -> Self {
		Self {
			unavailable: model_unavailable(model.as_ref()),
			key: level.key,
			label: level.label,
			hint: level.hint,
			model_id: level.model_id,
			reasoning: level.reasoning,
			model: model.map(|model| model.model),
		}
	}
}

impl PresetResponse {
	/// Builds the response for `viewer`, with `level_models` holding the model of each level in order; the instructions
	/// are hidden from viewers who may not see them.
	///
	/// A preset with levels is usable while at least one of its levels is; an arena only while every level is.
	#[must_use]
	pub fn build(mut preset: Preset, base: Option<PresetBaseModel>, level_models: Vec<Option<PresetBaseModel>>, viewer: &PresetActor) -> Self {
		let can_manage = viewer.can_manage(&preset);
		let can_edit = viewer.can_edit(&preset);
		let shared = !can_manage && !viewer.is_home(&preset);
		let scope = preset.scope();
		let team = preset.team_id.zip(preset.team_name.clone()).map(|(id, name)| PresetTeam { id, name });
		let levels: Vec<PresetLevelResponse> = std::mem::take(&mut preset.levels.0).into_iter().zip(level_models).map(|(level, model)| PresetLevelResponse::build(level, model)).collect();
		let is_arena = preset.is_arena();
		let unavailable = if !preset.is_enabled {
			Some(PresetUnavailable::Disabled)
		} else if is_arena {
			levels.iter().find_map(|level| level.unavailable)
		} else if !levels.is_empty() && levels.iter().any(|level| level.unavailable.is_none()) {
			None
		} else {
			model_unavailable(base.as_ref())
		};
		let key = preset.key();
		let system_prompt = if can_edit || preset.prompt_visible { preset.system_prompt } else { None };
		Self {
			id: preset.id,
			key,
			slug: preset.slug,
			scope,
			team,
			name: preset.name,
			description: preset.description,
			icon: preset.icon,
			base_model_id: preset.base_model_id,
			base_model: base.map(|base| base.model),
			system_prompt,
			sampling: preset.sampling.0,
			reasoning: preset.reasoning.0,
			levels,
			default_level: preset.default_level,
			tool_ids: preset.tool_ids,
			tools_locked: preset.tools_locked,
			starters: preset.starters,
			prompt_visible: preset.prompt_visible,
			is_enabled: preset.is_enabled,
			disabled_by_admin: preset.disabled_by_admin,
			mode: if is_arena { PresetMode::Arena } else { PresetMode::Standard },
			arena: preset.arena.0,
			has_gateway_usage: false,
			can_edit,
			can_manage,
			shared,
			unavailable,
			updated_at: preset.updated_at,
		}
	}
}
