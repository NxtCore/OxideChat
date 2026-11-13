use super::{Role, RoleActor, RoleError, RoleResponse, SaveRoleRequest};
use crate::logging::AuditLog;
use crate::types::consts::ADMIN_ROLES_EDIT;
use crate::types::{AuditEntry, User, is_unique_violation};
use serde_json::{Value, json};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
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

/// Whether some role grants `admin.roles.edit`, and whether some user holds such a role.
async fn role_managers(tx: &mut Transaction<'_, Postgres>) -> Result<(bool, bool), RoleError> {
	let granted: Vec<(String, bool)> = sqlx::query_as(
		"SELECT p.name, EXISTS (SELECT 1 FROM user_roles ur WHERE ur.role_id = rp.role_id) \
		 FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id",
	)
	.fetch_all(&mut **tx)
	.await?;
	let managing: Vec<bool> = granted.into_iter().filter(|(permission, _)| User::permission_matches(permission, ADMIN_ROLES_EDIT)).map(|(_, held)| held).collect();
	Ok((!managing.is_empty(), managing.contains(&true)))
}

impl Role {
	/// Serializes with role edits and returns whether some user holds `admin.roles.edit` before `user_roles` change.
	///
	/// # Errors
	/// Returns the database error when the lock or the lookup fails.
	pub(crate) async fn lock_managers(tx: &mut Transaction<'_, Postgres>) -> Result<bool, RoleError> {
		sqlx::query("LOCK TABLE role_permissions IN SHARE ROW EXCLUSIVE MODE").execute(&mut **tx).await?;
		Ok(role_managers(tx).await?.1)
	}

	/// Checks after a change to roles or `user_roles` that some role still grants `admin.roles.edit` and that someone can
	/// still manage roles if someone could before.
	///
	/// # Errors
	/// Returns [`RoleError::LockOut`] when nobody could manage roles any more.
	pub(crate) async fn check_managers_kept(tx: &mut Transaction<'_, Postgres>, held_before: bool) -> Result<(), RoleError> {
		let (granted, held) = role_managers(tx).await?;
		if granted && (held || !held_before) { Ok(()) } else { Err(RoleError::LockOut) }
	}
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

impl RoleActor<'_> {
	/// Checks that the actor holds every permission of the roles a user gains or loses when `current` becomes `requested`.
	///
	/// # Errors
	/// Returns [`RoleError::PermissionNotHeld`] when the actor lacks one of them, and the database error when the lookup fails.
	pub async fn check_role_change(&self, executor: impl PgExecutor<'_>, current: &[String], requested: &[String]) -> Result<(), RoleError> {
		let changed: Vec<String> = current
			.iter()
			.filter(|name| !requested.contains(name))
			.chain(requested.iter().filter(|name| !current.contains(name)))
			.cloned()
			.collect();
		check_held(self, &Role::permissions_of(executor, &changed).await?)
	}

	/// Checks that the actor holds every permission of `target`, so nobody can take over or remove a stronger account.
	///
	/// # Errors
	/// Returns [`RoleError::Outranked`] when the target holds a permission the actor does not, and the database error
	/// when the lookup fails.
	pub async fn check_outranks(&self, executor: impl PgExecutor<'_>, target: &User) -> Result<(), RoleError> {
		check_held(self, &target.permissions(executor).await?).map_err(|_| RoleError::Outranked)
	}
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

	/// Renames a role and replaces its permissions, audited in the same transaction. Built-in roles keep their name.
	///
	/// # Errors
	/// Returns [`RoleError::Protected`] when a built-in role would be renamed, [`RoleError::LockOut`] when no role could
	/// manage roles afterwards, [`RoleError::NotFound`] when it does not exist, and the errors of [`Role::create`] for a
	/// bad request.
	pub async fn update(pool: &PgPool, actor: &RoleActor<'_>, id: &Uuid, request: &SaveRoleRequest) -> Result<RoleResponse, RoleError> {
		let name = request.valid_name()?;
		let permissions = request.unique_permissions();
		check_held(actor, &permissions)?;
		let mut tx = pool.begin().await?;
		let before = Self::lock_changeable(&mut tx, actor, id).await?;
		if before.is_builtin && before.name != name {
			return Err(RoleError::Protected);
		}
		let (_, held_before) = role_managers(&mut tx).await?;
		check_known(&mut tx, &permissions).await?;
		sqlx::query("UPDATE roles SET name = $2 WHERE id = $1")
			.bind(id)
			.bind(name)
			.execute(&mut *tx)
			.await
			.map_err(map_write_error)?;
		replace_permissions(&mut tx, id, &permissions).await?;
		Self::check_managers_kept(&mut tx, held_before).await?;
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
	/// Returns [`RoleError::Protected`] for a built-in role, [`RoleError::LockOut`] when no role could manage roles
	/// afterwards, [`RoleError::PermissionNotHeld`] when the actor does not hold all of its permissions, and
	/// [`RoleError::NotFound`] when it does not exist.
	pub async fn delete(pool: &PgPool, actor: &RoleActor<'_>, id: &Uuid) -> Result<(), RoleError> {
		let mut tx = pool.begin().await?;
		let before = Self::lock_changeable(&mut tx, actor, id).await?;
		if before.is_builtin {
			return Err(RoleError::Protected);
		}
		let (_, held_before) = role_managers(&mut tx).await?;
		sqlx::query("DELETE FROM roles WHERE id = $1").bind(id).execute(&mut *tx).await?;
		Self::check_managers_kept(&mut tx, held_before).await?;
		let metadata = json!({ "name": before.name, "permissions": before.permissions, "user_count": before.user_count });
		audit(&mut tx, EVENT_DELETED, actor.id, id, metadata).await?;
		tx.commit().await?;
		Ok(())
	}

	async fn lock_changeable(tx: &mut Transaction<'_, Postgres>, actor: &RoleActor<'_>, id: &Uuid) -> Result<RoleResponse, RoleError> {
		sqlx::query("LOCK TABLE role_permissions IN SHARE ROW EXCLUSIVE MODE").execute(&mut **tx).await?;
		let locked: Option<Uuid> = sqlx::query_scalar("SELECT id FROM roles WHERE id = $1 FOR UPDATE")
			.bind(id)
			.fetch_optional(&mut **tx)
			.await?;
		if locked.is_none() {
			return Err(RoleError::NotFound);
		}
		let role = Self::find_detailed(&mut **tx, id).await?.ok_or(RoleError::NotFound)?;
		check_held(actor, &role.permissions)?;
		Ok(role)
	}
}
