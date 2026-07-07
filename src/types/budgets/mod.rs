use crate::types::BaseType;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Budget {
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
}

#[derive(Debug, Clone)]
pub struct EffectiveBudget {
	pub budget: Budget,
	pub assignment_id: Uuid,
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub assigned_at: DateTime<Utc>,
}

impl BaseType for Budget {}
