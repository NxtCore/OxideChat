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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
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
}

impl ProviderKind {
	#[must_use]
	pub fn to_omni_kind(&self) -> OmniProviderKind {
		match self {
			Self::Openai => OmniProviderKind::OpenAI,
			Self::OpenaiCompat => OmniProviderKind::OpenAICompat,
			Self::Anthropic => OmniProviderKind::Anthropic,
			Self::Google => OmniProviderKind::Google,
			Self::Openrouter => OmniProviderKind::OpenRouter,
			Self::Custom => OmniProviderKind::Custom("CUSTOM".to_string()),
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
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
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
