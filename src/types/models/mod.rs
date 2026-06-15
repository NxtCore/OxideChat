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

pub use requests::*;
pub use responses::*;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Model {
	pub id: Uuid,
	pub provider_id: Uuid,
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
}

pub struct ModelViewer<'a> {
	pub user_id: &'a Uuid,
}

pub struct ModelSyncInput {
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Value,
	pub input_modalities: Value,
	pub output_modalities: Value,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
}

pub struct ModelSyncSummary {
	pub added: usize,
	pub updated: usize,
	pub removed: usize,
}

impl BaseType for Model {}
