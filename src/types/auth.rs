//! Authentication-related types.
//!
//! Request and response types for user authentication and session management.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

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

// ============================================================================
// Internal Types (DB rows)
// ============================================================================

/// User database row.
#[derive(Debug, FromRow)]
pub struct User {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub password_hash: Option<String>,
	pub auth_method: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

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
