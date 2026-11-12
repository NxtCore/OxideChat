use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{Team, TeamRole};
use crate::types::TeamProviderRouting;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "TeamSummary"))]
pub struct TeamSummaryResponse {
	pub id: Uuid,
	pub name: String,
	pub is_default: bool,
}

/// A team one user belongs to, with the role they hold in it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserTeamResponse {
	pub id: Uuid,
	pub name: String,
	pub is_default: bool,
	pub role: TeamRole,
}

/// How a team grants models: all of them, every model of a provider, or one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ModelGrantKind {
	All,
	Provider,
	Model,
}

impl ModelGrantKind {
	/// Parses the kind a query returned; unknown values are treated as a single model.
	#[must_use]
	pub fn from_stored(value: &str) -> Self {
		match value {
			"all" => Self::All,
			"provider" => Self::Provider,
			_ => Self::Model,
		}
	}
}

/// One grant that lets a user use models: the team it comes from and the provider or model it names (`target` is empty for all models).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserModelGrant {
	pub team: TeamSummaryResponse,
	pub kind: ModelGrantKind,
	pub target: Option<ModelGrantTarget>,
}

/// The provider or model a grant names.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelGrantTarget {
	pub id: Uuid,
	pub name: String,
}

/// The models a user may use through their teams: how many of the enabled models, and the grants behind them.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserModelAccessResponse {
	pub usable: i64,
	pub total: i64,
	pub grants: Vec<UserModelGrant>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "TeamMember"))]
pub struct TeamMemberResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub role: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TeamModelAccessResponse {
	pub provider_ids: Vec<Uuid>,
	pub model_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "TeamList"))]
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
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "TeamDetailed"))]
pub struct TeamDetailedResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub is_default: bool,
	pub allow_all_models: bool,
	pub budget_id: Option<Uuid>,
	pub default_model_id: Option<Uuid>,
	pub members: Vec<TeamMemberResponse>,
	pub model_access: TeamModelAccessResponse,
	pub provider_routing: TeamProviderRouting,
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
