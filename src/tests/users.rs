#[cfg(test)]
mod overview {
	use crate::tests::support::{TestResult, create_user};
	use crate::types::{
		AssignmentKind, AssignmentRef, EffectivePermission, ModelGrantKind, PermissionResponse, PermissionSource, ReplaceAssignmentsRequest, RoleActor, RoleError, RoleResponse, TeamRole, User,
	};
	use chrono::{Duration, Utc};
	use sqlx::PgPool;
	use uuid::Uuid;

	fn held(permissions: &[&str]) -> Vec<String> {
		permissions.iter().map(|permission| (*permission).to_owned()).collect()
	}

	fn admin() -> Vec<String> {
		held(&["admin.*", "settings.*"])
	}

	fn role(name: &str, permissions: &[&str]) -> RoleResponse {
		RoleResponse {
			id: Uuid::new_v4(),
			name: name.to_owned(),
			is_builtin: false,
			permissions: held(permissions),
			user_count: 1,
		}
	}

	fn replace(roles: &[Uuid], teams: &[Uuid]) -> ReplaceAssignmentsRequest {
		let target = |kind: AssignmentKind, id: &Uuid| AssignmentRef { kind, id: Some(*id), role: None };
		ReplaceAssignmentsRequest {
			targets: roles
				.iter()
				.map(|id| target(AssignmentKind::Role, id))
				.chain(teams.iter().map(|id| target(AssignmentKind::Team, id)))
				.collect(),
		}
	}

	async fn find(pool: &PgPool, id: &Uuid) -> TestResult<User> {
		Ok(User::find_by_id(pool, id).await?.ok_or("user")?)
	}

	async fn member(pool: &PgPool) -> TestResult<User> {
		let id = create_user(pool).await?;
		let user = find(pool, &id).await?;
		user.initialize_defaults(pool).await?;
		user.set_roles(pool, &["user".to_owned()]).await?;
		Ok(user)
	}

	async fn role_id(pool: &PgPool, name: &str) -> TestResult<Uuid> {
		Ok(sqlx::query_scalar("SELECT id FROM roles WHERE name = $1").bind(name).fetch_one(pool).await?)
	}

	async fn create_role(pool: &PgPool, name: &str, permissions: &[&str]) -> TestResult<Uuid> {
		let id: Uuid = sqlx::query_scalar("INSERT INTO roles (name) VALUES ($1) RETURNING id").bind(name).fetch_one(pool).await?;
		sqlx::query("INSERT INTO role_permissions (role_id, permission_id) SELECT $1, id FROM permissions WHERE name = ANY($2)")
			.bind(id)
			.bind(held(permissions))
			.execute(pool)
			.await?;
		Ok(id)
	}

	async fn create_team(pool: &PgPool, name: &str) -> TestResult<Uuid> {
		Ok(sqlx::query_scalar("INSERT INTO teams (name) VALUES ($1) RETURNING id").bind(name).fetch_one(pool).await?)
	}

	async fn create_model(pool: &PgPool, name: &str) -> TestResult<(Uuid, Uuid)> {
		let provider: Uuid = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', $1, 'https://example.com/v1', true) RETURNING id")
			.bind(name)
			.fetch_one(pool)
			.await?;
		let model: Uuid = sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, $2, $2, true) RETURNING id")
			.bind(provider)
			.bind(name)
			.fetch_one(pool)
			.await?;
		Ok((provider, model))
	}

	async fn audit_count(pool: &PgPool, user_id: &Uuid) -> TestResult<i64> {
		Ok(
			sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE event = 'user_assignments_updated' AND target_id = $1")
				.bind(user_id)
				.fetch_one(pool)
				.await?,
		)
	}

	#[test]
	fn effective_permissions_name_the_role_and_the_wildcard_that_grant_them() {
		let all: Vec<PermissionResponse> = ["admin.*", "admin.users.edit", "admin.users.view", "settings.sessions.view"]
			.into_iter()
			.map(|name| PermissionResponse::new(name.to_owned()))
			.collect();
		let roles = [role("admin", &["admin.*"]), role("Support", &["admin.users.view", "admin.*"]), role("user", &[])];

		let effective = EffectivePermission::resolve(all, &roles);

		let names: Vec<&str> = effective.iter().map(|permission| permission.name.as_str()).collect();
		assert_eq!(names, ["admin.*", "admin.users.edit", "admin.users.view"]);
		let source = |role: &str, through: &str| PermissionSource {
			role: role.to_owned(),
			through: through.to_owned(),
		};
		assert_eq!(effective[1].sources, [source("admin", "admin.*"), source("Support", "admin.*")]);
		assert_eq!(effective[2].sources, [source("admin", "admin.*"), source("Support", "admin.users.view")]);
		assert_eq!(effective[2].group, "admin_users");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn overview_lists_everything_assigned_to_a_user(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let permissions = admin();
		let actor = RoleActor {
			id: &actor_id,
			permissions: &permissions,
		};
		let user = member(&pool).await?;
		let session = Uuid::new_v4();
		User::insert_session(&pool, session, &user.id, Utc::now() + Duration::days(1)).await?;
		let team = create_team(&pool, "Research").await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'admin')")
			.bind(team)
			.bind(user.id)
			.execute(&pool)
			.await?;
		let (provider, model) = create_model(&pool, "overview-model").await?;
		sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)").bind(team).bind(model).execute(&pool).await?;
		sqlx::query("INSERT INTO team_model_access (team_id, provider_id) VALUES ($1, $2)").bind(team).bind(provider).execute(&pool).await?;
		let budget: Uuid = sqlx::query_scalar("INSERT INTO budgets (name, amount, kind, interval) VALUES ('Research pool', 25, 'pooled', 'monthly') RETURNING id")
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, team_id) VALUES ($1, $2)").bind(budget).bind(team).execute(&pool).await?;

		let overview = user.overview(&pool, &actor, None).await?;

		assert_eq!(overview.profile.username, user.username);
		assert!(overview.can_edit);
		assert_eq!(overview.sessions.len(), 1);
		assert_eq!(overview.sessions[0].id, User::session_public_id(&session));
		assert_eq!(overview.roles.iter().map(|role| role.name.as_str()).collect::<Vec<_>>(), ["user"]);
		assert!(overview.permissions.iter().all(|permission| permission.name.starts_with("settings.") && permission.sources[0].role == "user"));
		assert!(!overview.permissions.is_empty());
		let teams: Vec<(&str, bool, TeamRole)> = overview.teams.iter().map(|team| (team.name.as_str(), team.is_default, team.role)).collect();
		assert_eq!(teams, [("General", true, TeamRole::Member), ("Research", false, TeamRole::Admin)]);
		assert_eq!(overview.assignments.kinds, [AssignmentKind::Role, AssignmentKind::Team]);
		let inherited: Vec<&str> = overview.assignments.targets.iter().filter(|target| target.inherited).map(|target| target.label.as_str()).collect();
		assert_eq!(inherited, ["General"]);

		let access = overview.model_access.ok_or("model access")?;
		assert_eq!((access.usable, access.total), (1, 1));
		let grants: Vec<(&str, ModelGrantKind, Option<&str>)> = access
			.grants
			.iter()
			.map(|grant| (grant.team.name.as_str(), grant.kind, grant.target.as_ref().map(|target| target.name.as_str())))
			.collect();
		assert_eq!(
			grants,
			[
				("General", ModelGrantKind::All, None),
				("Research", ModelGrantKind::Model, Some("overview-model")),
				("Research", ModelGrantKind::Provider, Some("overview-model")),
			]
		);
		let budgets = overview.budgets.ok_or("budgets")?;
		assert_eq!(budgets.len(), 1);
		assert_eq!((budgets[0].budget.name.as_str(), budgets[0].team_id), ("Research pool", Some(team)));
		assert_eq!(overview.presets.map(|presets| presets.len()), Some(0));
		assert_eq!(overview.keys.map(|keys| keys.len()), Some(0));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn overview_hides_the_sections_of_areas_the_viewer_may_not_see(pool: PgPool) -> TestResult {
		let viewer_id = create_user(&pool).await?;
		let permissions = held(&["admin.users.view"]);
		let viewer = RoleActor {
			id: &viewer_id,
			permissions: &permissions,
		};
		let user = member(&pool).await?;

		let overview = user.overview(&pool, &viewer, None).await?;

		assert!(!overview.can_edit);
		assert!(overview.assignments.kinds.is_empty());
		assert!(!overview.teams.is_empty());
		assert!(overview.model_access.is_none() && overview.presets.is_none() && overview.budgets.is_none() && overview.keys.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn replacing_assignments_changes_roles_and_teams_with_one_audit_row(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let permissions = admin();
		let actor = RoleActor {
			id: &actor_id,
			permissions: &permissions,
		};
		let user = member(&pool).await?;
		let user_role = role_id(&pool, "user").await?;
		let support = create_role(&pool, "Support", &["admin.users.view"]).await?;
		let research = create_team(&pool, "Research").await?;
		let design = create_team(&pool, "Design").await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'admin')")
			.bind(research)
			.bind(user.id)
			.execute(&pool)
			.await?;

		user.replace_assignments(&pool, &actor, &replace(&[user_role, support], &[research, design])).await?;

		let stored = user.assignments(&pool, &actor).await?;
		let labels: Vec<&str> = stored.targets.iter().map(|target| target.label.as_str()).collect();
		assert_eq!(labels, ["user", "Support", "General", "Design", "Research"]);
		assert!(user.check_permission(&pool, "admin.users.view").await?);
		let research_role: String = sqlx::query_scalar("SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2")
			.bind(research)
			.bind(user.id)
			.fetch_one(&pool)
			.await?;
		assert_eq!(research_role, "admin");
		assert_eq!(audit_count(&pool, &user.id).await?, 1);

		user.replace_assignments(&pool, &actor, &replace(&[user_role, support], &[research, design])).await?;
		assert_eq!(audit_count(&pool, &user.id).await?, 1);

		user.replace_assignments(&pool, &actor, &replace(&[user_role], &[])).await?;
		let labels: Vec<String> = user.assignments(&pool, &actor).await?.targets.into_iter().map(|target| target.label).collect();
		assert_eq!(labels, ["user", "General"]);
		assert_eq!(audit_count(&pool, &user.id).await?, 2);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn replacing_assignments_refuses_what_the_actor_may_not_change(pool: PgPool) -> TestResult {
		let user_role = role_id(&pool, "user").await?;
		let admin_role = role_id(&pool, "admin").await?;
		let team = create_team(&pool, "Research").await?;
		let strong = member(&pool).await?;
		strong.set_roles(&pool, &["user".to_owned(), "admin".to_owned()]).await?;
		let weak = member(&pool).await?;
		let limited = held(&["admin.users.edit", "admin.users.view", "settings.*"]);
		let weak_actor = RoleActor {
			id: &weak.id,
			permissions: &limited,
		};
		let full = admin();
		let strong_actor = RoleActor {
			id: &strong.id,
			permissions: &full,
		};
		let other = member(&pool).await?;

		let own = strong.replace_assignments(&pool, &strong_actor, &replace(&[user_role], &[])).await;
		assert!(matches!(own, Err(RoleError::OwnRoles)));
		strong.replace_assignments(&pool, &strong_actor, &replace(&[user_role, admin_role], &[team])).await?;
		assert_eq!(strong.assignments(&pool, &strong_actor).await?.kinds, [AssignmentKind::Team]);

		let grant = other.replace_assignments(&pool, &weak_actor, &replace(&[user_role, admin_role], &[])).await;
		assert!(matches!(grant, Err(RoleError::PermissionNotHeld)));
		let demote = strong.replace_assignments(&pool, &weak_actor, &replace(&[user_role], &[team])).await;
		assert!(matches!(demote, Err(RoleError::Outranked)));
		let move_out = strong.replace_assignments(&pool, &weak_actor, &replace(&[user_role, admin_role], &[])).await;
		assert!(matches!(move_out, Err(RoleError::Outranked)));
		assert!(strong.assignments(&pool, &weak_actor).await?.kinds.is_empty());

		let unknown = other.replace_assignments(&pool, &strong_actor, &replace(&[user_role], &[Uuid::new_v4()])).await;
		assert!(matches!(unknown, Err(RoleError::Database(_))));
		let labels: Vec<String> = other.assignments(&pool, &strong_actor).await?.targets.into_iter().map(|target| target.label).collect();
		assert_eq!(labels, ["user", "General"]);
		assert_eq!(audit_count(&pool, &other.id).await?, 0);
		Ok(())
	}
}
