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
	WHERE ($1::uuid IS NULL OR r.id = $1) 	AND ($2::uuid IS NULL OR EXISTS (SELECT 1 FROM user_roles held WHERE held.role_id = r.id AND held.user_id = $2)) \
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
		let rows = sqlx::query_as::<_, RoleRow>(ROLE_SELECT).bind(None::<Uuid>).bind(None::<Uuid>).fetch_all(pool).await?;
		Ok(rows.into_iter().map(RoleResponse::from).collect())
	}

	/// The roles a user holds with their permissions, built-in roles first.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_for_user(executor: impl PgExecutor<'_>, user_id: &Uuid) -> Result<Vec<RoleResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, RoleRow>(ROLE_SELECT).bind(None::<Uuid>).bind(user_id).fetch_all(executor).await?;
		Ok(rows.into_iter().map(RoleResponse::from).collect())
	}

	/// One role with its permissions and how many users hold it.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_detailed(executor: impl PgExecutor<'_>, id: &Uuid) -> Result<Option<RoleResponse>, sqlx::Error> {
		let row = sqlx::query_as::<_, RoleRow>(ROLE_SELECT).bind(id).bind(None::<Uuid>).fetch_optional(executor).await?;
		Ok(row.map(RoleResponse::from))
	}

	/// The names of the roles with the given IDs; IDs without a role are skipped.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn names_of(executor: impl PgExecutor<'_>, ids: &[Uuid]) -> Result<Vec<String>, sqlx::Error> {
		sqlx::query_scalar("SELECT name FROM roles WHERE id = ANY($1)").bind(ids).fetch_all(executor).await
	}

	/// The permissions granted by the roles with the given names.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn permissions_of(executor: impl PgExecutor<'_>, names: &[String]) -> Result<Vec<String>, sqlx::Error> {
		sqlx::query_scalar(
			"SELECT DISTINCT p.name FROM roles r JOIN role_permissions rp ON rp.role_id = r.id JOIN permissions p ON p.id = rp.permission_id WHERE r.name = ANY($1)",
		)
		.bind(names)
		.fetch_all(executor)
		.await
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
