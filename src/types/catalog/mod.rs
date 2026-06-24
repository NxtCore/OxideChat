use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

mod repository;
mod responses;
mod rows;

pub use responses::*;

/// Model-level availability of a gateway catalog model for the configured provider key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "gateway_availability", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AvailabilityState {
	Available,
	UserUnavailable,
}

/// A persisted gateway catalog model. Exists independently of runnable `models`; a
/// `USER_UNAVAILABLE` row has no `local_model_id`.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GatewayCatalogModel {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub source_gateway: String,
	pub gateway_model_id: String,
	pub local_model_id: Option<Uuid>,
	pub display_name: String,
	pub availability_state: AvailabilityState,
	pub reason: Option<String>,
	pub raw: Json<Value>,
	pub fetched_at: Option<DateTime<Utc>>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl BaseType for GatewayCatalogModel {}

/// Upsert input for a public catalog model (one per `/v1/models` row).
pub struct GatewayCatalogSyncInput {
	pub gateway_model_id: String,
	pub display_name: String,
	pub raw: Value,
}

/// Upsert input for a single provider endpoint option of a catalog model.
pub struct GatewayProviderOptionSyncInput {
	pub provider_slug: Option<String>,
	pub provider_name: Option<String>,
	pub endpoint_name: Option<String>,
	pub status: Option<f64>,
	pub quantization: Option<String>,
	pub context_length: Option<i32>,
	pub max_completion_tokens: Option<i32>,
	pub latency: Option<f64>,
	pub throughput: Option<f64>,
	pub uptime: Option<f64>,
	pub price_input: Option<f64>,
	pub price_output: Option<f64>,
	pub raw: Value,
}
