use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListBudgetsQuery {
	pub page: Option<i64>,
	pub size: Option<i64>,
	pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudgetRequest {
	pub name: String,
	pub description: Option<String>,
	pub amount: Decimal,
	pub kind: String,
	pub interval: String,
	pub reset_strategy: String,
	pub on_exceed: String,
	pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBudgetRequest {
	pub name: Option<String>,
	pub description: Option<Option<String>>,
	pub amount: Option<Decimal>,
	pub kind: Option<String>,
	pub interval: Option<String>,
	pub reset_strategy: Option<String>,
	pub on_exceed: Option<String>,
	pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BudgetAssignmentRequest {
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct BudgetResetRequest {
	pub assignment_id: Option<Uuid>,
	pub budget_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub kind: Option<String>,
	pub reason: Option<String>,
}
