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

	#[must_use]
	pub fn from_omni_kind(kind: &OmniProviderKind) -> Self {
		match kind {
			OmniProviderKind::OpenAI => Self::Openai,
			OmniProviderKind::OpenAICompat => Self::OpenaiCompat,
			OmniProviderKind::Anthropic => Self::Anthropic,
			OmniProviderKind::Google => Self::Google,
			OmniProviderKind::OpenRouter => Self::Openrouter,
			OmniProviderKind::Custom(_) => Self::Custom,
		}
	}

	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Openai => "OPENAI",
			Self::OpenaiCompat => "OPENAI_COMPAT",
			Self::Openrouter => "OPENROUTER",
			Self::Anthropic => "ANTHROPIC",
			Self::Google => "GOOGLE",
			Self::Custom => "CUSTOM",
		}
	}

	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"OPENAI" => Some(Self::Openai),
			"OPENAI_COMPAT" => Some(Self::OpenaiCompat),
			"OPENROUTER" => Some(Self::Openrouter),
			"ANTHROPIC" => Some(Self::Anthropic),
			"GOOGLE" => Some(Self::Google),
			"CUSTOM" => Some(Self::Custom),
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
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProviderSlim {
	pub id: Uuid,
	pub name: String,
	pub kind: ProviderKind,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderBillingConnection {
	pub provider_id: Uuid,
	pub credential: Option<String>,
	pub last_status: String,
	pub last_error_code: Option<String>,
	pub last_synced_at: Option<DateTime<Utc>>,
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
	pub details: Option<Value>,
	pub fetched_at: Option<DateTime<Utc>>,
}

impl BaseType for Provider {}
