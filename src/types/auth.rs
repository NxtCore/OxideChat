//! Authentication-related types.
//!
//! Request and response types for user authentication and session management.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{PreferencesResponse, User};

// ============================================================================
// Request Types
// ============================================================================

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

/// Request body for setting a user's full role list.
#[derive(Debug, Deserialize)]
pub struct SetUserRolesRequest {
	pub roles: Vec<String>,
}

/// Query parameters for listing users.
#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
	pub page: Option<i64>,
	pub per_page: Option<i64>,
	pub search: Option<String>,
	pub role: Option<String>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Sanitized user response (no password hash).
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

// ============================================================================
// Internal Types (DB rows)
// ============================================================================

/// Role database row.
#[derive(Debug, FromRow)]
pub struct Role {
	pub id: Uuid,
	pub name: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Session database row.
#[derive(Debug, FromRow)]
pub struct Session {
	pub id: Uuid,
	pub user_id: Uuid,
	pub expires_at: chrono::DateTime<chrono::Utc>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// User identity for external auth providers.
#[derive(Debug, FromRow)]
pub struct UserIdentity {
	pub id: Uuid,
	pub user_id: Uuid,
	pub provider: String,
	pub provider_user_id: String,
	pub provider_data: Option<serde_json::Value>,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Helper for counting rows.
#[derive(Debug, FromRow)]
pub struct CountRow {
	pub count: i64,
}

/// Helper for role names query.
#[derive(Debug, FromRow)]
pub struct RoleNameRow {
	pub name: String,
}

/// Helper for permission names query.
#[derive(Debug, FromRow)]
pub struct PermissionNameRow {
	pub name: String,
}
