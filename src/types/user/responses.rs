use crate::types::PreferencesResponse;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct UserResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub auth_method: String,
	pub roles: Vec<String>,
	pub permissions: Vec<String>,
	pub preferences: PreferencesResponse,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserListResponse {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub auth_method: String,
	pub roles: Vec<String>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedUsersResponse {
	pub users: Vec<UserResponse>,
	pub total: i64,
	pub page: i64,
	pub per_page: i64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedUsersListResponse {
	pub users: Vec<UserListResponse>,
	pub total: i64,
	pub page: i64,
	pub per_page: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
	pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
	pub message: String,
}
