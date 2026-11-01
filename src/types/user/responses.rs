use crate::types::PreferencesResponse;
use crate::types::teams::TeamSummaryResponse;
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
