use super::rows::{CountRow, RoleRow};
use super::{PermissionResponse, Role, RoleResponse};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

const ROLE_SELECT: &str = "SELECT r.id, r.name, r.is_builtin, \
	COALESCE(ARRAY_AGG(p.name ORDER BY p.name) FILTER (WHERE p.name IS NOT NULL), '{}') AS permissions, \
	(SELECT COUNT(*) FROM user_roles ur WHERE ur.role_id = r.id) AS user_count \
	FROM roles r \
	LEFT JOIN role_permissions rp ON rp.role_id = r.id \
	LEFT JOIN permissions p ON p.id = rp.permission_id \
	WHERE ($1::uuid IS NULL OR r.id = $1) \
	GROUP BY r.id \
	ORDER BY r.is_builtin DESC, lower(r.name)";

impl Role {
	pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
		let count: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM roles WHERE name = $1")
			.bind(name)
			.fetch_one(pool)
			.await?;
		Ok(count.count > 0)
	}

	pub async fn list_all(pool: &PgPool) -> Result<Vec<Role>, sqlx::Error> {
		sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name ASC").fetch_all(pool).await
	}

	/// Every role with its permissions and how many users hold it, built-in roles first.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_detailed(pool: &PgPool) -> Result<Vec<RoleResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, RoleRow>(ROLE_SELECT).bind(None::<Uuid>).fetch_all(pool).await?;
		Ok(rows.into_iter().map(RoleResponse::from).collect())
	}

	/// One role with its permissions and how many users hold it.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_detailed(executor: impl PgExecutor<'_>, id: &Uuid) -> Result<Option<RoleResponse>, sqlx::Error> {
		let row = sqlx::query_as::<_, RoleRow>(ROLE_SELECT).bind(id).fetch_optional(executor).await?;
		Ok(row.map(RoleResponse::from))
	}

	/// Every permission that can be granted, sorted by name, with its group.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_permissions(pool: &PgPool) -> Result<Vec<PermissionResponse>, sqlx::Error> {
		let names: Vec<String> = sqlx::query_scalar("SELECT name FROM permissions ORDER BY name").fetch_all(pool).await?;
		Ok(names.into_iter().map(PermissionResponse::new).collect())
	}
}
