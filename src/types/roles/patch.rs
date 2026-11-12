use super::{Role, RoleActor, RoleError, RoleResponse, SaveRoleRequest};
use crate::logging::AuditLog;
use crate::types::{AuditEntry, is_unique_violation};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const AUDIT_TARGET: &str = "role";
const EVENT_CREATED: &str = "role_created";
const EVENT_UPDATED: &str = "role_updated";
const EVENT_DELETED: &str = "role_deleted";

fn map_write_error(error: sqlx::Error) -> RoleError {
	if is_unique_violation(&error) {
		RoleError::NameTaken
	} else {
		RoleError::Database(error)
	}
}

fn check_held(actor: &RoleActor<'_>, permissions: &[String]) -> Result<(), RoleError> {
	if permissions.iter().all(|permission| actor.holds(permission)) {
		Ok(())
	} else {
		Err(RoleError::PermissionNotHeld)
	}
}

async fn check_known(tx: &mut Transaction<'_, Postgres>, permissions: &[String]) -> Result<(), RoleError> {
	let known: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE name = ANY($1)")
		.bind(permissions)
		.fetch_one(&mut **tx)
		.await?;
	if usize::try_from(known).is_ok_and(|known| known == permissions.len()) {
		Ok(())
	} else {
		Err(RoleError::UnknownPermission)
	}
}

async fn replace_permissions(tx: &mut Transaction<'_, Postgres>, role_id: &Uuid, permissions: &[String]) -> Result<(), sqlx::Error> {
	sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
		.bind(role_id)
		.execute(&mut **tx)
		.await?;
	sqlx::query("INSERT INTO role_permissions (role_id, permission_id) SELECT $1, id FROM permissions WHERE name = ANY($2)")
		.bind(role_id)
		.bind(permissions)
		.execute(&mut **tx)
		.await?;
	Ok(())
}

async fn audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, role_id: &Uuid, metadata: Value) -> Result<(), sqlx::Error> {
	let entry = AuditEntry {
		event,
		actor_id,
		target_type: AUDIT_TARGET,
		target_id: role_id,
		metadata,
	};
	AuditLog::write(tx, entry).await
}

impl Role {
	/// Creates a role with the given permissions, audited in the same transaction.
	///
	/// # Errors
	/// Returns [`RoleError::InvalidName`], [`RoleError::NameTaken`], [`RoleError::UnknownPermission`] or
	/// [`RoleError::PermissionNotHeld`] when the role cannot be created as asked.
	pub async fn create(pool: &PgPool, actor: &RoleActor<'_>, request: &SaveRoleRequest) -> Result<RoleResponse, RoleError> {
		let name = request.valid_name()?;
		let permissions = request.unique_permissions();
		check_held(actor, &permissions)?;
		let mut tx = pool.begin().await?;
		check_known(&mut tx, &permissions).await?;
		let id: Uuid = sqlx::query_scalar("INSERT INTO roles (name) VALUES ($1) RETURNING id")
			.bind(name)
			.fetch_one(&mut *tx)
			.await
			.map_err(map_write_error)?;
		replace_permissions(&mut tx, &id, &permissions).await?;
		audit(&mut tx, EVENT_CREATED, actor.id, &id, json!({ "name": name, "permissions": permissions })).await?;
		let role = Self::find_detailed(&mut *tx, &id).await?.ok_or(RoleError::NotFound)?;
		tx.commit().await?;
		Ok(role)
	}

	/// Renames a role and replaces its permissions, audited in the same transaction.
	///
	/// # Errors
	/// Returns [`RoleError::Protected`] for a built-in role, [`RoleError::NotFound`] when it does not exist, and the
	/// errors of [`Role::create`] for a bad request.
	pub async fn update(pool: &PgPool, actor: &RoleActor<'_>, id: &Uuid, request: &SaveRoleRequest) -> Result<RoleResponse, RoleError> {
		let name = request.valid_name()?;
		let permissions = request.unique_permissions();
		check_held(actor, &permissions)?;
		let mut tx = pool.begin().await?;
		let before = Self::lock_changeable(&mut tx, actor, id).await?;
		check_known(&mut tx, &permissions).await?;
		sqlx::query("UPDATE roles SET name = $2 WHERE id = $1")
			.bind(id)
			.bind(name)
			.execute(&mut *tx)
			.await
			.map_err(map_write_error)?;
		replace_permissions(&mut tx, id, &permissions).await?;
		let metadata = json!({
			"name": name,
			"permissions": permissions,
			"before": { "name": before.name, "permissions": before.permissions },
		});
		audit(&mut tx, EVENT_UPDATED, actor.id, id, metadata).await?;
		let role = Self::find_detailed(&mut *tx, id).await?.ok_or(RoleError::NotFound)?;
		tx.commit().await?;
		Ok(role)
	}

	/// Deletes a role; its users lose it. Audited in the same transaction.
	///
	/// # Errors
	/// Returns [`RoleError::Protected`] for a built-in role, [`RoleError::PermissionNotHeld`] when the actor does not
	/// hold all of its permissions, and [`RoleError::NotFound`] when it does not exist.
	pub async fn delete(pool: &PgPool, actor: &RoleActor<'_>, id: &Uuid) -> Result<(), RoleError> {
		let mut tx = pool.begin().await?;
		let before = Self::lock_changeable(&mut tx, actor, id).await?;
		sqlx::query("DELETE FROM roles WHERE id = $1").bind(id).execute(&mut *tx).await?;
		let metadata = json!({ "name": before.name, "permissions": before.permissions, "user_count": before.user_count });
		audit(&mut tx, EVENT_DELETED, actor.id, id, metadata).await?;
		tx.commit().await?;
		Ok(())
	}

	async fn lock_changeable(tx: &mut Transaction<'_, Postgres>, actor: &RoleActor<'_>, id: &Uuid) -> Result<RoleResponse, RoleError> {
		let locked: Option<Uuid> = sqlx::query_scalar("SELECT id FROM roles WHERE id = $1 FOR UPDATE")
			.bind(id)
			.fetch_optional(&mut **tx)
			.await?;
		if locked.is_none() {
			return Err(RoleError::NotFound);
		}
		let role = Self::find_detailed(&mut **tx, id).await?.ok_or(RoleError::NotFound)?;
		if role.is_builtin {
			return Err(RoleError::Protected);
		}
		check_held(actor, &role.permissions)?;
		Ok(role)
	}
}
