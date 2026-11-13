//! A user's roles and teams in the shared assignment shape.

use super::User;
use crate::logging::AuditLog;
use crate::types::consts::ADMIN_USERS_EDIT;
use crate::types::{
	AUDIT_USER_TARGET, AssignmentKind, AssignmentTarget, AssignmentsResponse, AuditEntry, EVENT_USER_ASSIGNMENTS_UPDATED, ReplaceAssignmentsRequest, Role, RoleActor, RoleError, RoleResponse,
	Team, UserTeamResponse,
};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

impl RoleActor<'_> {
	/// Whether the actor may change a user who holds `roles`: they need `admin.users.edit` and every permission the user holds.
	#[must_use]
	pub fn can_edit_user(&self, roles: &[RoleResponse]) -> bool {
		self.holds(ADMIN_USERS_EDIT) && roles.iter().flat_map(|role| &role.permissions).all(|permission| self.holds(permission))
	}
}

impl User {
	/// A user is assigned to roles and teams.
	pub const ASSIGNMENT_KINDS: [AssignmentKind; 2] = [AssignmentKind::Role, AssignmentKind::Team];

	/// Builds the assignments from loaded roles and teams; the default team is `inherited` because every user belongs to it, and nobody
	/// may change their own roles.
	#[must_use]
	pub fn assignments_from(&self, actor: &RoleActor<'_>, roles: &[RoleResponse], teams: &[UserTeamResponse]) -> AssignmentsResponse {
		let kinds = if !actor.can_edit_user(roles) {
			Vec::new()
		} else if actor.id == &self.id {
			vec![AssignmentKind::Team]
		} else {
			Self::ASSIGNMENT_KINDS.to_vec()
		};
		let role_targets = roles.iter().map(|role| (AssignmentKind::Role, role.id, role.name.clone(), false));
		let team_targets = teams.iter().map(|team| (AssignmentKind::Team, team.id, team.name.clone(), team.is_default));
		AssignmentsResponse {
			kinds,
			single: false,
			roles: Vec::new(),
			targets: role_targets.chain(team_targets).map(AssignmentTarget::from).collect(),
		}
	}

	/// The roles and teams of this user, with the kinds `actor` may change.
	///
	/// # Errors
	/// Returns the database error when a query fails.
	pub async fn assignments(&self, pool: &PgPool, actor: &RoleActor<'_>) -> Result<AssignmentsResponse, sqlx::Error> {
		let roles = Role::list_for_user(pool, &self.id).await?;
		let teams = Team::memberships_for_user(pool, &self.id).await?;
		Ok(self.assignments_from(actor, &roles, &teams))
	}

	/// Makes the requested roles and teams the only ones of this user, audited in one transaction; the default team always stays.
	///
	/// # Errors
	/// Returns [`RoleError::OwnRoles`] when the actor changes their own roles, [`RoleError::PermissionNotHeld`] when a role that is
	/// gained or lost grants something the actor does not hold, [`RoleError::Outranked`] when the roles or teams of a stronger account change,
	/// and the database error when a role or team does not exist; nothing is changed then.
	pub async fn replace_assignments(&self, pool: &PgPool, actor: &RoleActor<'_>, request: &ReplaceAssignmentsRequest) -> Result<(), RoleError> {
		let role_ids = request.ids(AssignmentKind::Role);
		let team_ids = request.ids(AssignmentKind::Team);
		let mut tx = pool.begin().await?;
		let held_before = Role::lock_managers(&mut tx).await?;
		sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE").bind(self.id).execute(&mut *tx).await?;
		let current = Role::list_for_user(&mut *tx, &self.id).await?;
		let roles_change = current.len() != role_ids.len() || current.iter().any(|role| !role_ids.contains(&role.id));
		if roles_change && actor.id == &self.id {
			return Err(RoleError::OwnRoles);
		}
		if roles_change || self.teams_change(&mut tx, &team_ids).await? {
			actor.check_outranks(&mut *tx, self).await?;
		}
		if roles_change {
			let current_names: Vec<String> = current.into_iter().map(|role| role.name).collect();
			let requested_names = Role::names_of(&mut *tx, &role_ids).await?;
			actor.check_role_change(&mut *tx, &current_names, &requested_names).await?;
		}

		let roles = self.replace_roles(&mut tx, &role_ids, roles_change, held_before).await?;
		let teams = Team::replace_user_teams(&mut tx, &self.id, &team_ids).await?;
		self.audit_assignments(&mut tx, actor.id, roles, teams).await?;
		tx.commit().await?;
		Ok(())
	}

	async fn audit_assignments(&self, tx: &mut Transaction<'_, Postgres>, actor_id: &Uuid, roles: (Vec<Uuid>, Vec<Uuid>), teams: (Vec<Uuid>, Vec<Uuid>)) -> Result<(), sqlx::Error> {
		let ((roles_removed, roles_added), (teams_left, teams_joined)) = (roles, teams);
		if roles_removed.is_empty() && roles_added.is_empty() && teams_left.is_empty() && teams_joined.is_empty() {
			return Ok(());
		}
		let entry = AuditEntry {
			event: EVENT_USER_ASSIGNMENTS_UPDATED,
			actor_id,
			target_type: AUDIT_USER_TARGET,
			target_id: &self.id,
			metadata: json!({
				"roles": { "added": roles_added, "removed": roles_removed },
				"teams": { "added": teams_joined, "removed": teams_left },
			}),
		};
		AuditLog::write(tx, entry).await
	}

	async fn teams_change(&self, tx: &mut Transaction<'_, Postgres>, team_ids: &[Uuid]) -> Result<bool, sqlx::Error> {
		let memberships = Team::memberships_for_user(&mut **tx, &self.id).await?;
		let leaves = memberships.iter().any(|team| !team.is_default && !team_ids.contains(&team.id));
		Ok(leaves || team_ids.iter().any(|id| memberships.iter().all(|team| team.id != *id)))
	}

	async fn replace_roles(&self, tx: &mut Transaction<'_, Postgres>, role_ids: &[Uuid], changed: bool, held_before: bool) -> Result<(Vec<Uuid>, Vec<Uuid>), RoleError> {
		if !changed {
			return Ok((Vec::new(), Vec::new()));
		}
		let removed = sqlx::query_scalar("DELETE FROM user_roles WHERE user_id = $1 AND role_id <> ALL($2) RETURNING role_id")
			.bind(self.id)
			.bind(role_ids)
			.fetch_all(&mut **tx)
			.await?;
		let added = sqlx::query_scalar(
			"INSERT INTO user_roles (user_id, role_id) SELECT $1, role_id FROM unnest($2::uuid[]) AS role_id ON CONFLICT DO NOTHING RETURNING role_id",
		)
		.bind(self.id)
		.bind(role_ids)
		.fetch_all(&mut **tx)
		.await?;
		Role::check_managers_kept(tx, held_before).await?;
		Ok((removed, added))
	}
}
