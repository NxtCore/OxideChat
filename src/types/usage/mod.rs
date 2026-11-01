use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod chat_guard;
mod repository;

pub use chat_guard::ChatUsageGuard;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageEvent {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub provider_id: Option<Uuid>,
	pub request_type: String,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
	pub created_at: DateTime<Utc>,
}

pub struct UsageEventRecord<'a> {
	pub user_id: Option<&'a Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub model_id: &'a Uuid,
	pub provider_id: &'a Uuid,
	pub request_type: &'a str,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UsageFilter {
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
	pub group_by: Option<String>,
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
}

impl AnalyticsQuery {
	#[must_use]
	pub fn filter(&self) -> UsageFilter {
		UsageFilter {
			user_id: self.user_id,
			team_id: self.team_id,
			api_key_id: self.api_key_id,
		}
	}
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AnalyticsRow {
	pub id: Option<Uuid>,
	pub label: String,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub reasoning_tokens: i64,
	#[cfg_attr(test, ts(type = "string"))]
	pub cost_total: Decimal,
	pub request_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ApiKeyAnalyticsRow {
	pub id: Option<Uuid>,
	pub name: Option<String>,
	pub last_four: Option<String>,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub reasoning_tokens: i64,
	#[cfg_attr(test, ts(type = "string"))]
	pub cost_total: Decimal,
	pub request_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AnalyticsDayModelRow {
	pub day: String,
	pub model_id: Option<Uuid>,
	pub model_name: String,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub reasoning_tokens: i64,
	#[cfg_attr(test, ts(type = "string"))]
	pub cost_total: Decimal,
	pub request_count: i64,
}
