use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub(super) struct RoleNameRow {
	pub name: String,
}

#[derive(Debug, FromRow)]
pub(super) struct PermissionNameRow {
	pub name: String,
}

#[derive(Debug, FromRow)]
pub(super) struct CountRow {
	pub count: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct UserRoleRow {
	pub user_id: Uuid,
	pub name: String,
}
