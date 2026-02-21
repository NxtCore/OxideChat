use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::CountRow;

/// Request body for setting a user's full role list.
#[derive(Debug, Deserialize)]
pub struct SetUserRolesRequest {
	pub roles: Vec<String>,
}
/// Role database row.
#[derive(Debug, Serialize, FromRow)]
pub struct Role {
	pub id: Uuid,
	pub name: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Helper for role names query.
#[derive(Debug, FromRow)]
pub struct RoleNameRow {
	pub name: String,
}
 
impl Role {
	/// Check if a role name exists in the database.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn exists(pool: &sqlx::PgPool, name: &str) -> Result<bool, sqlx::Error> {
		let count: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM roles WHERE name = $1")
			.bind(name)
			.fetch_one(pool)
			.await?;
		Ok(count.count > 0)
	}

	/// Get all roles from the database.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn list_all(pool: &sqlx::PgPool) -> Result<Vec<Role>, sqlx::Error> {
		sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name ASC")
			.fetch_all(pool)
			.await
	}
}
