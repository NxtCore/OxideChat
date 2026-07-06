use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

const VALID_KINDS: [&str; 2] = ["pooled", "per_user"];
const VALID_INTERVALS: [&str; 4] = ["daily", "weekly", "monthly", "total"];
const VALID_RESET_STRATEGIES: [&str; 3] = ["rolling", "anchored", "calendar"];
const VALID_ON_EXCEED: [&str; 3] = ["block", "warn", "allow"];

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

impl CreateBudgetRequest {
	#[must_use]
	pub fn has_valid_enums(&self) -> bool {
		VALID_KINDS.contains(&self.kind.as_str())
			&& VALID_INTERVALS.contains(&self.interval.as_str())
			&& VALID_RESET_STRATEGIES.contains(&self.reset_strategy.as_str())
			&& VALID_ON_EXCEED.contains(&self.on_exceed.as_str())
	}
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

impl UpdateBudgetRequest {
	#[must_use]
	pub fn has_valid_enums(&self) -> bool {
		self.kind.as_deref().is_none_or(|kind| VALID_KINDS.contains(&kind))
			&& self.interval.as_deref().is_none_or(|interval| VALID_INTERVALS.contains(&interval))
			&& self.reset_strategy.as_deref().is_none_or(|reset_strategy| VALID_RESET_STRATEGIES.contains(&reset_strategy))
			&& self.on_exceed.as_deref().is_none_or(|on_exceed| VALID_ON_EXCEED.contains(&on_exceed))
	}
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
