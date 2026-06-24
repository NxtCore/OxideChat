use super::{AvailabilityState, GatewayCatalogModelResponse, GatewayProviderOption};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct GatewayCatalogModelRow {
	pub id: Uuid,
	pub gateway_model_id: String,
	pub display_name: String,
	pub availability_state: AvailabilityState,
	pub reason: Option<String>,
	pub local_model_id: Option<Uuid>,
	pub fetched_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
pub(super) struct GatewayProviderOptionRow {
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

impl From<GatewayCatalogModelRow> for GatewayCatalogModelResponse {
	fn from(row: GatewayCatalogModelRow) -> Self {
		Self {
			id: row.id,
			gateway_model_id: row.gateway_model_id,
			display_name: row.display_name,
			availability_state: row.availability_state,
			reason: row.reason,
			local_model_id: row.local_model_id,
			fetched_at: row.fetched_at,
		}
	}
}

impl From<GatewayProviderOptionRow> for GatewayProviderOption {
	fn from(row: GatewayProviderOptionRow) -> Self {
		Self {
			id: row.id,
			provider_slug: row.provider_slug,
			provider_name: row.provider_name,
			endpoint_name: row.endpoint_name,
			status: row.status,
			quantization: row.quantization,
			context_length: row.context_length,
			max_completion_tokens: row.max_completion_tokens,
			latency: row.latency,
			throughput: row.throughput,
			uptime: row.uptime,
			price_input: row.price_input,
			price_output: row.price_output,
		}
	}
}
