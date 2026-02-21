//! Authentication-related types.
//!
//! Request and response types for user authentication and session management.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{PreferencesResponse, UserResponse};

/// Request body for initial admin setup.
#[derive(Debug, Deserialize)]
pub struct SetupRequest {
	pub email: String,
	pub username: String,
	pub password: String,
}

/// Request body for user registration.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
	pub email: String,
	pub username: String,
	pub password: String,
}

/// Request body for user login.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
	pub email: String,
	pub password: String,
}

/// Request body for admin user creation.
#[derive(Debug, Deserialize)]
pub struct CreateAdminUserRequest {
	pub email: String,
	pub username: String,
	pub password: String,
	pub roles: Vec<String>,
}

/// Request body for admin user update.
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
	pub username: Option<String>,
	pub email: Option<String>,
}

/// Request body for admin-initiated password reset.
#[derive(Debug, Deserialize)]
pub struct AdminResetPasswordRequest {
	pub password: String,
}

/// Query parameters for listing users.
#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
	pub page: Option<i64>,
	pub per_page: Option<i64>,
	pub search: Option<String>,
	pub role: Option<String>,
}

/// Response for successful authentication.
#[derive(Debug, Serialize)]
pub struct AuthResponse {
	pub user: UserResponse,
}

/// Simple message response.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
	pub message: String,
}

/// Paginated list of users.
#[derive(Debug, Serialize)]
pub struct PaginatedUsersResponse {
	pub users: Vec<UserResponse>,
	pub total: i64,
	pub page: i64,
	pub per_page: i64,
}

/// Helper for counting rows.
#[derive(Debug, FromRow)]
pub struct CountRow {
	pub count: i64,
}

/// Helper for permission names query.
#[derive(Debug, FromRow)]
pub struct PermissionNameRow {
	pub name: String,
}
