use serde::{Deserialize, Deserializer};
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
	pub page: Option<i64>,
	pub size: Option<i64>,
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
	pub default_model_key: Option<Option<String>>,
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
pub struct UpdateTeamBudgetRequest {
	pub budget_id: Option<Uuid>,
}
