use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
	pub page: Option<i64>,
	pub per_page: Option<i64>,
	pub search: Option<String>,
	pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdminUserRequest {
	pub email: String,
	pub username: String,
	pub password: String,
	pub roles: Vec<String>,
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
	pub roles: Vec<String>,
}
