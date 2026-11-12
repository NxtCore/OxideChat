//! Everything that applies to one user, as admins see it on the user's page.

use super::{EffectivePermission, PermissionSource, User, UserOverviewResponse, UserProfileResponse};
use crate::types::consts::{ADMIN_BUDGETS_VIEW, ADMIN_DEVELOPER_VIEW, ADMIN_PRESETS_VIEW, ADMIN_TEAMS_VIEW};
use crate::types::{Budget, BudgetSubject, GatewayAdminKeyResponse, PermissionResponse, Preset, Role, RoleActor, RoleResponse, Team};
use sqlx::PgPool;
use uuid::Uuid;

impl EffectivePermission {
	/// Keeps the permissions that one of `roles` grants, each with the roles granting it; a role that holds the permission itself and a
	/// wildcard covering it is listed once, with the permission itself.
	#[must_use]
	pub fn resolve(all: Vec<PermissionResponse>, roles: &[RoleResponse]) -> Vec<Self> {
		all.into_iter()
			.filter_map(|permission| {
				let sources: Vec<PermissionSource> = roles
					.iter()
					.filter_map(|role| {
						let exact = role.permissions.iter().find(|held| **held == permission.name);
						let through = exact.or_else(|| role.permissions.iter().find(|held| User::permission_matches(held, &permission.name)))?;
						Some(PermissionSource {
							role: role.name.clone(),
							through: through.clone(),
						})
					})
					.collect();
				(!sources.is_empty()).then_some(Self {
					name: permission.name,
					group: permission.group,
					sources,
				})
			})
			.collect()
	}
}

impl User {
	/// Loads everything that applies to this user for `actor`, who is signed in with `session`.
	///
	/// Model access, presets, budgets and API keys belong to other admin areas and are only loaded when the actor may view that area.
	///
	/// # Errors
	/// Returns the database error when a query fails.
	pub async fn overview(&self, pool: &PgPool, actor: &RoleActor<'_>, session: Option<&Uuid>) -> Result<UserOverviewResponse, sqlx::Error> {
		let roles = Role::list_for_user(pool, &self.id).await?;
		let teams = Team::memberships_for_user(pool, &self.id).await?;
		let permissions = EffectivePermission::resolve(Role::list_permissions(pool).await?, &roles);
		let model_access = if actor.holds(ADMIN_TEAMS_VIEW) { Some(Team::model_access_for_user(pool, &self.id).await?) } else { None };
		let presets = if actor.holds(ADMIN_PRESETS_VIEW) { Some(Preset::admin_list_visible_to(pool, &self.id).await?) } else { None };
		let budgets = if actor.holds(ADMIN_BUDGETS_VIEW) { Some(Budget::evaluate(pool, BudgetSubject::user(self.id)).await?) } else { None };
		let keys = if actor.holds(ADMIN_DEVELOPER_VIEW) { Some(GatewayAdminKeyResponse::list_for_user(pool, &self.id).await?) } else { None };
		Ok(UserOverviewResponse {
			profile: UserProfileResponse {
				id: self.id,
				email: self.email.clone(),
				username: self.username.clone(),
				auth_method: self.auth_method.clone(),
				created_at: self.created_at,
			},
			can_edit: actor.can_edit_user(&roles),
			sessions: Self::list_sessions(pool, &self.id, session).await?,
			assignments: self.assignments_from(actor, &roles, &teams),
			roles,
			permissions,
			teams,
			model_access,
			presets,
			budgets,
			keys,
		})
	}
}
