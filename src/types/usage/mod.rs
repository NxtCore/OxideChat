use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod repository;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageEvent {
	pub id: Uuid,
	pub user_id: Uuid,
	pub team_id: Option<Uuid>,
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
	pub user_id: &'a Uuid,
	pub team_id: Option<Uuid>,
	pub model_id: &'a Uuid,
	pub provider_id: &'a Uuid,
	pub request_type: &'a str,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
	pub group_by: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AnalyticsRow {
	pub id: Option<Uuid>,
	pub label: String,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub reasoning_tokens: i64,
	pub cost_total: Decimal,
	pub request_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AnalyticsDayModelRow {
	pub day: String,
	pub model_id: Option<Uuid>,
	pub model_name: String,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub reasoning_tokens: i64,
	pub cost_total: Decimal,
	pub request_count: i64,
}
