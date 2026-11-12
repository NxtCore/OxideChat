use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

pub const MAX_ROLE_NAME_LEN: usize = 50;

#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Role {
	pub id: Uuid,
	pub name: String,
	pub is_builtin: bool,
	pub created_at: DateTime<Utc>,
}

impl BaseType for Role {}

/// Who changes a role, with the permissions they hold; nobody may grant or take away a permission they do not hold.
pub struct RoleActor<'a> {
	pub id: &'a Uuid,
	pub permissions: &'a [String],
}

impl RoleActor<'_> {
	/// Whether the actor holds `permission` directly or through a wildcard.
	#[must_use]
	pub fn holds(&self, permission: &str) -> bool {
		self.permissions.iter().any(|held| crate::types::User::permission_matches(held, permission))
	}
}

#[derive(Debug, thiserror::Error)]
pub enum RoleError {
	#[error("the role name is empty or too long")]
	InvalidName,
	#[error("the role name is already in use")]
	NameTaken,
	#[error("a permission does not exist")]
	UnknownPermission,
	#[error("built-in roles cannot be changed")]
	Protected,
	#[error("the actor does not hold every permission involved")]
	PermissionNotHeld,
	#[error("the role does not exist")]
	NotFound,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}
