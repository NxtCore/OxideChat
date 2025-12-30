//! Permission-related types.
//!
//! Types for the extensible permission system.

use serde::Serialize;
use sqlx::FromRow;

/// Permission name row from database.
#[derive(Debug, FromRow)]
pub struct PermissionNameRow {
	pub name: String,
}

/// Permission response for API.
#[derive(Debug, Serialize)]
pub struct PermissionInfo {
	pub name: String,
	pub description: Option<String>,
}

/// Permission constants for compile-time safety.
pub mod consts {
	pub const PROFILE_VIEW: &str = "settings.profile.view";
	pub const PROFILE_EDIT: &str = "settings.profile.edit";
	pub const SESSIONS_VIEW: &str = "settings.sessions.view";
	pub const SESSIONS_REVOKE: &str = "settings.sessions.revoke";
	pub const APPEARANCE_EDIT: &str = "settings.appearance.edit";
	pub const ADMIN_USERS_VIEW: &str = "admin.users.view";
	pub const ADMIN_USERS_EDIT: &str = "admin.users.edit";
	pub const ADMIN_CONFIG_VIEW: &str = "admin.config.view";
	pub const ADMIN_CONFIG_EDIT: &str = "admin.config.edit";
}
