use super::{ModelDetailed, ModelListAdmin, ModelListPublic};
use crate::types::providers::{ProviderKind, ProviderSlim};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct ModelListPublicRow {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Json<Vec<String>>,
	pub input_modalities: Json<Vec<String>>,
	pub output_modalities: Json<Vec<String>>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider_id: Uuid,
	pub provider_name: String,
	pub provider_kind: ProviderKind,
	pub icon: Option<String>,
	pub is_favorite: bool,
}

#[derive(sqlx::FromRow)]
pub(super) struct ModelListAdminRow {
	pub id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Json<Vec<String>>,
	pub input_modalities: Json<Vec<String>>,
	pub output_modalities: Json<Vec<String>>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub provider_id: Uuid,
	pub provider_name: String,
	pub provider_kind: ProviderKind,
	pub icon: Option<String>,
}

pub(super) struct ModelDetailedRow {
	pub id: Uuid,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Json<Vec<String>>,
	pub input_modalities: Json<Vec<String>>,
	pub output_modalities: Json<Vec<String>>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub provider_id: Uuid,
	pub provider_name: String,
	pub provider_kind: ProviderKind,
	pub icon: Option<String>,
	pub description: Option<String>,
	pub system_prompt: Option<String>,
	pub sampling: Option<Json<Value>>,
	pub extra_settings: Option<Json<Value>>,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub is_favorite: bool,
	pub category: Option<String>,
	pub tags: Json<Vec<String>>,
}

impl From<ModelListPublicRow> for ModelListPublic {
	fn from(row: ModelListPublicRow) -> Self {
		Self {
			id: row.id,
			model_id: row.model_id,
			display_name: row.display_name,
			capabilities: row.capabilities.0,
			input_modalities: row.input_modalities.0,
			output_modalities: row.output_modalities.0,
			context_length: row.context_length,
			max_tokens: row.max_tokens,
			is_enabled: row.is_enabled,
			provider: ProviderSlim {
				id: row.provider_id,
				name: row.provider_name,
				kind: row.provider_kind,
			},
			icon: row.icon,
			is_favorite: row.is_favorite,
			budget_blocked: false,
		}
	}
}

impl From<ModelListAdminRow> for ModelListAdmin {
	fn from(row: ModelListAdminRow) -> Self {
		Self {
			id: row.id,
			model_id: row.model_id,
			display_name: row.display_name,
			capabilities: row.capabilities.0,
			input_modalities: row.input_modalities.0,
			output_modalities: row.output_modalities.0,
			context_length: row.context_length,
			max_tokens: row.max_tokens,
			is_enabled: row.is_enabled,
			created_at: row.created_at,
			updated_at: row.updated_at,
			provider: ProviderSlim {
				id: row.provider_id,
				name: row.provider_name,
				kind: row.provider_kind,
			},
			icon: row.icon,
		}
	}
}

impl From<ModelDetailedRow> for ModelDetailed {
	fn from(row: ModelDetailedRow) -> Self {
		Self {
			id: row.id,
			created_at: row.created_at,
			updated_at: row.updated_at,
			model_id: row.model_id,
			display_name: row.display_name,
			capabilities: row.capabilities.0,
			input_modalities: row.input_modalities.0,
			output_modalities: row.output_modalities.0,
			context_length: row.context_length,
			max_tokens: row.max_tokens,
			is_enabled: row.is_enabled,
			provider: ProviderSlim {
				id: row.provider_id,
				name: row.provider_name,
				kind: row.provider_kind,
			},
			icon: row.icon,
			description: row.description,
			system_prompt: row.system_prompt,
			sampling: row.sampling.map(|j| j.0),
			extra_settings: row.extra_settings.map(|j| j.0),
			is_public: row.is_public,
			is_featured: row.is_featured,
			is_default: row.is_default,
			is_favorite: row.is_favorite,
			category: row.category,
			tags: row.tags.0,
		}
	}
}
