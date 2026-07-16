use super::ProviderKind;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderBillingConnection {
	pub provider_id: Uuid,
	pub credential: Option<String>,
	pub external_scope_id: Option<String>,
	pub external_scope_name: Option<String>,
	pub is_enabled: bool,
	pub last_status: String,
	pub last_error_code: Option<String>,
	pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ProviderBillingMetric {
	pub metric_kind: super::ProviderBillingMetricKind,
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
	pub external_scope_id: Option<String>,
	pub external_scope_name: Option<String>,
	pub is_enabled: Option<bool>,
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
