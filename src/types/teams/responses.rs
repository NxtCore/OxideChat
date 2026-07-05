use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::Team;

#[derive(Debug, Clone, Serialize)]
pub struct TeamSummaryResponse {
	pub id: Uuid,
	pub name: String,
	pub is_default: bool,
}

#[derive(Debug, Serialize)]
pub struct TeamMemberResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
}

#[derive(Debug, Serialize)]
pub struct TeamModelAccessResponse {
	pub provider_ids: Vec<Uuid>,
	pub model_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub is_default: bool,
	pub allow_all_models: bool,
	pub budget_id: Option<Uuid>,
	pub member_count: i64,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TeamDetailedResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub is_default: bool,
	pub allow_all_models: bool,
	pub budget_id: Option<Uuid>,
	pub default_model_key: Option<String>,
	pub members: Vec<TeamMemberResponse>,
	pub model_access: TeamModelAccessResponse,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl From<Team> for TeamSummaryResponse {
	fn from(team: Team) -> Self {
		Self {
			id: team.id,
			name: team.name,
			is_default: team.is_default,
		}
	}
}
