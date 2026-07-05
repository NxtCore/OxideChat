use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use super::{Budget, EffectiveBudget};

#[derive(Debug, Serialize)]
pub struct BudgetResponse {
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

#[derive(Debug, Serialize)]
pub struct BudgetAssignmentResponse {
	pub id: Uuid,
	pub budget_id: Uuid,
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BudgetAssignmentInfo {
	pub id: Uuid,
	pub budget_id: Uuid,
	pub team_id: Option<Uuid>,
	pub team_name: Option<String>,
	pub user_id: Option<Uuid>,
	pub user_label: Option<String>,
	pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct EffectiveBudgetResponse {
	pub budget: BudgetResponse,
	pub assignment_id: Uuid,
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub amount: Decimal,
	pub spent: Decimal,
	pub remaining: Decimal,
	pub window_start: DateTime<Utc>,
	pub resets_at: Option<DateTime<Utc>>,
	pub on_exceed: String,
	pub exhausted: bool,
}

#[derive(Debug, Serialize)]
pub struct UserBudgetStatus {
	pub budgets: Vec<EffectiveBudgetResponse>,
	pub decision: String,
	pub blocked_model_ids: Vec<Uuid>,
}

impl From<Budget> for BudgetResponse {
	fn from(budget: Budget) -> Self {
		Self {
			id: budget.id,
			name: budget.name,
			description: budget.description,
			amount: budget.amount,
			kind: budget.kind,
			interval: budget.interval,
			reset_strategy: budget.reset_strategy,
			on_exceed: budget.on_exceed,
			is_enabled: budget.is_enabled,
			created_at: budget.created_at,
			updated_at: budget.updated_at,
		}
	}
}

impl From<&EffectiveBudget> for BudgetAssignmentResponse {
	fn from(budget: &EffectiveBudget) -> Self {
		Self {
			id: budget.assignment_id,
			budget_id: budget.budget.id,
			team_id: budget.team_id,
			user_id: budget.user_id,
			assigned_at: budget.assigned_at,
		}
	}
}
