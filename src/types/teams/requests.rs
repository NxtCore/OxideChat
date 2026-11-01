use serde::{Deserialize, Deserializer};
use super::TeamRole;
use uuid::Uuid;

fn deserialize_nullable_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
	T: Deserialize<'de>,
	D: Deserializer<'de>,
{
	Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
pub struct ListTeamsQuery {
	pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
	pub name: String,
	pub description: Option<String>,
	pub allow_all_models: Option<bool>,
	pub member_ids: Option<Vec<Uuid>>,
	pub provider_ids: Option<Vec<Uuid>>,
	pub model_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamRequest {
	pub name: Option<String>,
	pub description: Option<Option<String>>,
	pub allow_all_models: Option<bool>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub default_model_id: Option<Option<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamMembersRequest {
	pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamModelsRequest {
	pub provider_ids: Vec<Uuid>,
	pub model_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct TeamMemberInput {
	pub user_id: Uuid,
	pub role: TeamRole,
}

#[derive(Debug, Deserialize)]
pub struct SaveTeamRequest {
	pub name: String,
	pub description: Option<String>,
	pub allow_all_models: bool,
	pub default_model_id: Option<Uuid>,
	pub members: Vec<TeamMemberInput>,
	pub provider_ids: Vec<Uuid>,
	pub model_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamBudgetRequest {
	pub budget_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamMemberRoleRequest {
	pub role: TeamRole,
}
