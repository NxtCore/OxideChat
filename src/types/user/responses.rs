use crate::types::teams::TeamSummaryResponse;
use crate::types::{
	AdminPresetResponse, AssignmentsResponse, EffectiveBudgetResponse, GatewayAdminKeyResponse, PreferencesResponse, RoleResponse, UserModelAccessResponse, UserTeamResponse,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SessionResponse {
	pub id: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub expires_at: chrono::DateTime<chrono::Utc>,
	pub is_current: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub auth_method: String,
	pub roles: Vec<String>,
	pub teams: Vec<TeamSummaryResponse>,
	pub permissions: Vec<String>,
	pub preferences: PreferencesResponse,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserListResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub auth_method: String,
	pub roles: Vec<String>,
	pub teams: Vec<TeamSummaryResponse>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
	pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
	pub message: String,
}

/// The account itself.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserProfileResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub auth_method: String,
	pub created_at: DateTime<Utc>,
}

/// A role that grants a permission, and the permission of that role that covers it (a wildcard such as `admin.*`, or the permission itself).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PermissionSource {
	pub role: String,
	pub through: String,
}

/// A permission a user holds, with every role that grants it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EffectivePermission {
	pub name: String,
	pub group: String,
	pub sources: Vec<PermissionSource>,
}

/// One user with everything assigned to them; sections of an admin area the viewer may not see are `null`.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UserOverviewResponse {
	pub profile: UserProfileResponse,
	/// Whether the viewer may change the account: its profile, password, sessions, roles and teams.
	pub can_edit: bool,
	pub sessions: Vec<SessionResponse>,
	pub roles: Vec<RoleResponse>,
	pub permissions: Vec<EffectivePermission>,
	pub teams: Vec<UserTeamResponse>,
	/// The roles and teams as assignment targets, with the kinds the viewer may change.
	pub assignments: AssignmentsResponse,
	pub model_access: Option<UserModelAccessResponse>,
	pub presets: Option<Vec<AdminPresetResponse>>,
	pub budgets: Option<Vec<EffectiveBudgetResponse>>,
	pub keys: Option<Vec<GatewayAdminKeyResponse>>,
}
