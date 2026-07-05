use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct EffectiveBudgetRow {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub amount: Decimal,
	pub kind: String,
	pub interval: String,
	pub reset_strategy: String,
	pub on_exceed: String,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub assignment_id: Uuid,
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub assigned_at: DateTime<Utc>,
}
