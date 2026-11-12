use super::RoleResponse;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub(super) struct CountRow {
	pub count: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct RoleRow {
	pub id: Uuid,
	pub name: String,
	pub is_builtin: bool,
	pub permissions: Vec<String>,
	pub user_count: i64,
}

impl From<RoleRow> for RoleResponse {
	fn from(row: RoleRow) -> Self {
		Self {
			id: row.id,
			name: row.name,
			is_builtin: row.is_builtin,
			permissions: row.permissions,
			user_count: row.user_count,
		}
	}
}
