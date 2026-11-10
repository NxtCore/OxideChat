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

pub use omniference::types::ProviderKind as OmniProviderKind;
use rust_decimal::Decimal;
pub use requests::*;
pub use responses::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, sqlx::Type)]
#[sqlx(type_name = "provider_kind", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ProviderKind {
	Openai,
	OpenaiCompat,
	Openrouter,
	Anthropic,
	Google,
	Custom,
	Cortecs,
	Mistral,
}

impl ProviderKind {
	/// Kinds that route each request to one of several upstream providers. Their requests honour the provider
	/// selector and the admins' upstream provider allowlists through `ChatRequestIR::provider_routing`, which
	/// Omniference's adapter for the kind maps onto the provider's own routing fields.
	pub const UPSTREAM_ROUTING: &[Self] = &[Self::Openrouter];

	/// Whether requests to this kind can be routed to a chosen upstream provider.
	#[must_use]
	pub fn routes_upstream(&self) -> bool {
		Self::UPSTREAM_ROUTING.contains(self)
	}

	#[must_use]
	pub fn to_omni_kind(&self) -> OmniProviderKind {
		match self {
			Self::Openai => OmniProviderKind::OpenAI,
			Self::OpenaiCompat => OmniProviderKind::OpenAICompat,
			Self::Anthropic => OmniProviderKind::Anthropic,
			Self::Google => OmniProviderKind::Google,
			Self::Openrouter => OmniProviderKind::OpenRouter,
			Self::Custom => OmniProviderKind::Custom("CUSTOM".to_string()),
			Self::Cortecs | Self::Mistral => OmniProviderKind::OpenAI,
		}
	}

	/// The models.dev provider slug for kinds that share another kind's adapter, so prices are looked up for that provider.
	#[must_use]
	pub fn catalog_slug(&self) -> Option<String> {
		match self {
			Self::Cortecs => Some("cortecs".to_string()),
			Self::Mistral => Some("mistral".to_string()),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub api_key: Option<String>,
	pub extra_headers: Json<Value>,
	pub is_enabled: bool,
	pub reported_cost: Option<Json<ProviderReportedCost>>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProviderSlim {
	pub id: Uuid,
	pub name: String,
	pub kind: ProviderKind,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderBillingConnection {
	pub credential: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderBillingMetric {
	pub metric_kind: ProviderBillingMetricKind,
	pub currency: String,
	pub period_start: Option<DateTime<Utc>>,
	pub period_end: Option<DateTime<Utc>>,
	pub limit_amount: Option<Decimal>,
	pub spent_amount: Option<Decimal>,
	pub remaining_amount: Option<Decimal>,
	pub is_hard_limit: bool,
	pub thresholds: Vec<Decimal>,
	pub details: Value,
	pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ProviderBillingOverviewRow {
	pub provider_id: Uuid,
	pub provider_kind: ProviderKind,
	pub credential: Option<String>,
	pub has_provider_api_key: bool,
	pub last_status: Option<String>,
	pub last_error_code: Option<String>,
	pub last_synced_at: Option<DateTime<Utc>>,
	pub metric_kind: Option<String>,
	pub currency: Option<String>,
	pub period_start: Option<DateTime<Utc>>,
	pub period_end: Option<DateTime<Utc>>,
	pub limit_amount: Option<Decimal>,
	pub spent_amount: Option<Decimal>,
	pub remaining_amount: Option<Decimal>,
	pub is_hard_limit: Option<bool>,
	pub thresholds: Option<Value>,
	pub fetched_at: Option<DateTime<Utc>>,
}

impl BaseType for Provider {}
