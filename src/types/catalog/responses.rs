use super::AvailabilityState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// A catalog model row for the admin catalog/search view.
#[derive(Debug, Serialize)]
pub struct GatewayCatalogModelResponse {
	pub id: Uuid,
	pub gateway_model_id: String,
	pub display_name: String,
	pub availability_state: AvailabilityState,
	pub reason: Option<String>,
	pub local_model_id: Option<Uuid>,
	pub fetched_at: Option<DateTime<Utc>>,
}

/// A single provider endpoint option for a runnable model.
#[derive(Debug, Serialize)]
pub struct GatewayProviderOption {
	pub id: Uuid,
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
}

/// Provider options for a runnable model, plus the parent catalog availability that drives
/// row render state. `availability_state` is `None` when the model has no linked catalog row.
#[derive(Debug, Serialize)]
pub struct ModelProviderOptions {
	pub gateway_model_id: Option<String>,
	pub availability_state: Option<AvailabilityState>,
	pub options: Vec<GatewayProviderOption>,
}
