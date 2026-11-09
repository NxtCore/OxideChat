use super::{Preset, PresetReasoning, PresetSampling, PresetScope};
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
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub base_model_id: Option<Uuid>,
	pub base_model: Option<ModelListPublic>,
	pub system_prompt: Option<String>,
	pub sampling: PresetSampling,
	pub reasoning: PresetReasoning,
	pub tool_ids: Vec<Uuid>,
	pub tools_locked: bool,
	pub starters: Vec<String>,
	pub prompt_visible: bool,
	pub is_enabled: bool,
	pub can_edit: bool,
	pub unavailable: Option<PresetUnavailable>,
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

impl PresetResponse {
	/// Builds the response for `viewer`; the instructions are hidden from viewers who may not see them.
	#[must_use]
	pub fn build(preset: Preset, base: Option<PresetBaseModel>, viewer: &Uuid) -> Self {
		let can_edit = preset.is_owned_by(viewer);
		let unavailable = if !preset.is_enabled {
			Some(PresetUnavailable::Disabled)
		} else {
			match &base {
				None => Some(PresetUnavailable::BaseModelUnavailable),
				Some(base) if !base.allowed => Some(PresetUnavailable::BaseModelNotAllowed),
				Some(_) => None,
			}
		};
		let key = preset.key();
		let system_prompt = if can_edit || preset.prompt_visible { preset.system_prompt } else { None };
		Self {
			id: preset.id,
			key,
			slug: preset.slug,
			scope: PresetScope::from_stored(&preset.scope),
			name: preset.name,
			description: preset.description,
			icon: preset.icon,
			base_model_id: preset.base_model_id,
			base_model: base.map(|base| base.model),
			system_prompt,
			sampling: preset.sampling.0,
			reasoning: preset.reasoning.0,
			tool_ids: preset.tool_ids,
			tools_locked: preset.tools_locked,
			starters: preset.starters,
			prompt_visible: preset.prompt_visible,
			is_enabled: preset.is_enabled,
			can_edit,
			unavailable,
			updated_at: preset.updated_at,
		}
	}
}
