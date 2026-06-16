use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub use patch::ModelConfigPatchField;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelConfig {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub model_id: Uuid,
	pub stable_key: String,
	pub name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub capabilities: Option<Json<Vec<String>>>,
	pub input_modalities: Option<Json<Vec<String>>>,
	pub output_modalities: Option<Json<Vec<String>>>,
	pub context_length: Option<i32>,
	pub max_output_tokens: Option<i32>,
	pub system_prompt: Option<String>,
	pub sampling: Json<Value>,
	pub enabled_tools: Json<Vec<String>>,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub is_favorite: bool,
	pub category: Option<String>,
	pub tags: Json<Vec<String>>,
	pub usage_count: i32,
	pub extra_settings: Json<Value>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

pub struct ModelConfigViewer<'a> {
	pub user_id: &'a Uuid,
}

impl BaseType for ModelConfig {}
