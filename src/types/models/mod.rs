use crate::types::BaseType;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

mod patch;
mod pricing;
mod repository;
mod requests;
mod responses;
mod rows;

pub use pricing::*;
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

/// A model's price: the catalog's, the manual override and the merged price requests are billed at.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelPricing {
	pub model_id: Uuid,
	#[cfg_attr(test, ts(type = "string | null"))]
	pub reported_input: Option<Decimal>,
	#[cfg_attr(test, ts(type = "string | null"))]
	pub reported_output: Option<Decimal>,
	pub override_pricing: Option<PriceSheet>,
	#[cfg_attr(test, ts(type = "string"))]
	pub effective_input: Decimal,
	#[cfg_attr(test, ts(type = "string"))]
	pub effective_output: Decimal,
	/// The price Omniference's catalog knows for the model's provider.
	pub catalog: Option<PricedSheet>,
	/// The override on top of the catalog price.
	pub effective: Option<PricedSheet>,
	/// The full price Omniference calculates costs with.
	#[serde(skip)]
	#[cfg_attr(test, ts(skip))]
	pub pricing: Option<omniference::catalog::ModelPricing>,
}

impl BaseType for Model {}
