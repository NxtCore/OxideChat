use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
	pub page: Option<i64>,
	pub per_page: Option<i64>,
	pub search: Option<String>,
	pub role: Option<String>,
	pub team_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdminUserRequest {
	pub email: String,
	pub username: String,
	pub password: String,
	pub roles: Vec<String>,
	pub team_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
	pub username: Option<String>,
	pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminResetPasswordRequest {
	pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetUserRolesRequest {
	#[serde(default)]
	pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetUserTeamsRequest {
	#[serde(default)]
	pub team_ids: Vec<Uuid>,
}
