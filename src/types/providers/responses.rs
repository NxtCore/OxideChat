use super::{Provider, ProviderKind};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderBillingStatus {
	NotConfigured,
	Available,
	Unsupported,
	Unauthorized,
	UpstreamError,
}

impl ProviderBillingStatus {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::NotConfigured => "NOT_CONFIGURED",
			Self::Available => "AVAILABLE",
			Self::Unsupported => "UNSUPPORTED",
			Self::Unauthorized => "UNAUTHORIZED",
			Self::UpstreamError => "UPSTREAM_ERROR",
		}
	}

	#[must_use]
	pub fn from_str(value: &str) -> Option<Self> {
		match value {
			"NOT_CONFIGURED" => Some(Self::NotConfigured),
			"AVAILABLE" => Some(Self::Available),
			"UNSUPPORTED" => Some(Self::Unsupported),
			"UNAUTHORIZED" => Some(Self::Unauthorized),
			"UPSTREAM_ERROR" => Some(Self::UpstreamError),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderBillingMetricKind {
	CreditBalance,
	KeyLimit,
	SpendThreshold,
	SpendOnly,
}

impl ProviderBillingMetricKind {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::CreditBalance => "CREDIT_BALANCE",
			Self::KeyLimit => "KEY_LIMIT",
			Self::SpendThreshold => "SPEND_THRESHOLD",
			Self::SpendOnly => "SPEND_ONLY",
		}
	}

	#[must_use]
	pub fn from_str(value: &str) -> Option<Self> {
		match value {
			"CREDIT_BALANCE" => Some(Self::CreditBalance),
			"KEY_LIMIT" => Some(Self::KeyLimit),
			"SPEND_THRESHOLD" => Some(Self::SpendThreshold),
			"SPEND_ONLY" => Some(Self::SpendOnly),
			_ => None,
		}
	}
}

#[derive(Debug, Serialize)]
pub struct ProviderBillingOverviewResponse {
	pub provider_id: Uuid,
	pub provider_kind: ProviderKind,
	pub status: ProviderBillingStatus,
	pub is_enabled: bool,
	pub has_billing_credential: bool,
	pub external_scope_id: Option<String>,
	pub external_scope_name: Option<String>,
	pub upstream: Option<ProviderBillingMetricResponse>,
	pub local: ProviderLocalSpendResponse,
	pub last_synced_at: Option<DateTime<Utc>>,
	pub is_stale: bool,
	pub error_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderBillingMetricResponse {
	pub metric_kind: ProviderBillingMetricKind,
	pub currency: String,
	pub period_start: Option<DateTime<Utc>>,
	pub period_end: Option<DateTime<Utc>>,
	pub limit_amount: Option<Decimal>,
	pub spent_amount: Option<Decimal>,
	pub remaining_amount: Option<Decimal>,
	pub is_hard_limit: bool,
	pub thresholds: Vec<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct ProviderLocalSpendResponse {
	pub currency: String,
	pub period_start: DateTime<Utc>,
	pub period_end: DateTime<Utc>,
	pub spent_amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct ProviderResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub has_api_key: bool,
	pub extra_headers: Value,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProviderModelResponse {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct TestProviderResponse {
	pub success: bool,
	pub models_found: usize,
	pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SyncProviderResponse {
	pub success: bool,
	pub models_added: usize,
	pub models_updated: usize,
	pub models_removed: usize,
	pub message: String,
}

impl From<Provider> for ProviderResponse {
	fn from(provider: Provider) -> Self {
		Self {
			id: provider.id,
			owner_id: provider.owner_id,
			kind: provider.kind,
			name: provider.name,
			base_url: provider.base_url,
			has_api_key: provider.api_key.is_some(),
			extra_headers: provider.extra_headers.0,
			is_enabled: provider.is_enabled,
			created_at: provider.created_at,
			updated_at: provider.updated_at,
		}
	}
}
