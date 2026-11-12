#[cfg(test)]
mod tests {
	use crate::tests::support::{TestResult, create_user};
	use crate::types::permissions::ALL;
	use crate::types::{PermissionResponse, Role, RoleActor, RoleError, SaveRoleRequest, User};
	use sqlx::PgPool;
	use uuid::Uuid;

	fn request(name: &str, permissions: &[&str]) -> SaveRoleRequest {
		SaveRoleRequest {
			name: name.to_owned(),
			permissions: permissions.iter().map(|permission| (*permission).to_owned()).collect(),
		}
	}

	fn admin_permissions() -> Vec<String> {
		vec!["admin.*".to_owned(), "settings.*".to_owned()]
	}

	async fn builtin_id(pool: &PgPool, name: &str) -> TestResult<Uuid> {
		Ok(sqlx::query_scalar("SELECT id FROM roles WHERE name = $1").bind(name).fetch_one(pool).await?)
	}

	async fn audit_events(pool: &PgPool, role_id: &Uuid) -> TestResult<Vec<String>> {
		Ok(
			sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type = 'role' AND target_id = $1 ORDER BY xmin::text::bigint")
				.bind(role_id)
				.fetch_all(pool)
				.await?,
		)
	}

	#[test]
	fn permissions_are_grouped_by_area() {
		assert_eq!(PermissionResponse::group_of("admin.*"), "wildcard");
		assert_eq!(PermissionResponse::group_of("settings.*"), "wildcard");
		assert_eq!(PermissionResponse::group_of("admin.users.view"), "admin_users");
		assert_eq!(PermissionResponse::group_of("admin.developer.*"), "admin_developer");
		assert_eq!(PermissionResponse::group_of("admin.developer.keys.edit"), "admin_developer");
		assert_eq!(PermissionResponse::group_of("settings.sessions.revoke"), "settings");
	}

	#[test]
	fn names_are_trimmed_and_limited() {
		assert_eq!(request("  Support  ", &[]).valid_name().ok(), Some("Support"));
		assert!(matches!(request("   ", &[]).valid_name(), Err(RoleError::InvalidName)));
		assert!(matches!(request(&"x".repeat(51), &[]).valid_name(), Err(RoleError::InvalidName)));
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn every_permission_constant_can_be_granted(pool: PgPool) -> TestResult {
		let granted: Vec<String> = Role::list_permissions(&pool).await?.into_iter().map(|permission| permission.name).collect();
		for (_, permission) in ALL {
			assert!(granted.iter().any(|name| name == permission), "{permission} is missing from the permissions table");
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn roles_are_created_changed_and_deleted_with_audit(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let held = admin_permissions();
		let actor = RoleActor {
			id: &actor_id,
			permissions: &held,
		};

		let created = Role::create(&pool, &actor, &request(" Support ", &["admin.users.view", "admin.users.view", "admin.teams.view"])).await?;
		assert_eq!(created.name, "Support");
		assert_eq!(created.permissions, ["admin.teams.view", "admin.users.view"]);
		assert!(!created.is_builtin);

		let member_id = create_user(&pool).await?;
		let member = User::find_by_id(&pool, &member_id).await?.ok_or("member")?;
		member.set_roles(&pool, &["Support".to_owned()]).await?;
		assert!(member.check_permission(&pool, "admin.users.view").await?);
		assert!(!member.check_permission(&pool, "admin.users.edit").await?);

		let updated = Role::update(&pool, &actor, &created.id, &request("Helpdesk", &["admin.users.edit"])).await?;
		assert_eq!(updated.name, "Helpdesk");
		assert_eq!(updated.permissions, ["admin.users.edit"]);
		assert_eq!(updated.user_count, 1);
		assert!(member.check_permission(&pool, "admin.users.edit").await?);
		assert_eq!(member.roles(&pool).await?, ["Helpdesk"]);

		Role::delete(&pool, &actor, &created.id).await?;
		assert!(member.roles(&pool).await?.is_empty());
		assert!(Role::find_detailed(&pool, &created.id).await?.is_none());
		assert_eq!(audit_events(&pool, &created.id).await?, ["role_created", "role_updated", "role_deleted"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn builtin_roles_are_protected(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let held = admin_permissions();
		let actor = RoleActor {
			id: &actor_id,
			permissions: &held,
		};
		for name in ["admin", "user"] {
			let id = builtin_id(&pool, name).await?;
			assert!(matches!(Role::update(&pool, &actor, &id, &request(name, &[])).await, Err(RoleError::Protected)));
			assert!(matches!(Role::delete(&pool, &actor, &id).await, Err(RoleError::Protected)));
		}
		let roles = Role::list_detailed(&pool).await?;
		assert!(roles.iter().filter(|role| role.is_builtin).count() >= 2);
		assert!(
			roles
				.iter()
				.any(|role| role.name == "admin" && role.permissions.iter().any(|permission| permission == "admin.*"))
		);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn only_held_permissions_can_be_granted_or_removed(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let admin_held = admin_permissions();
		let admin = RoleActor {
			id: &actor_id,
			permissions: &admin_held,
		};
		let limited_held = vec!["admin.roles.edit".to_owned(), "admin.users.view".to_owned()];
		let limited = RoleActor {
			id: &actor_id,
			permissions: &limited_held,
		};

		assert!(matches!(
			Role::create(&pool, &limited, &request("Escalate", &["admin.users.edit"])).await,
			Err(RoleError::PermissionNotHeld)
		));
		assert!(matches!(
			Role::create(&pool, &limited, &request("Everything", &["admin.*"])).await,
			Err(RoleError::PermissionNotHeld)
		));
		let viewer = Role::create(&pool, &limited, &request("Viewer", &["admin.users.view"])).await?;

		let strong = Role::create(&pool, &admin, &request("Strong", &["admin.config.edit"])).await?;
		assert!(matches!(
			Role::update(&pool, &limited, &strong.id, &request("Weak", &[])).await,
			Err(RoleError::PermissionNotHeld)
		));
		assert!(matches!(Role::delete(&pool, &limited, &strong.id).await, Err(RoleError::PermissionNotHeld)));
		Role::delete(&pool, &limited, &viewer.id).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn bad_requests_are_refused(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let held = vec!["*".to_owned()];
		let actor = RoleActor {
			id: &actor_id,
			permissions: &held,
		};
		assert!(matches!(
			Role::create(&pool, &actor, &request("Ghost", &["admin.ghosts.view"])).await,
			Err(RoleError::UnknownPermission)
		));
		assert!(matches!(Role::create(&pool, &actor, &request("admin", &[])).await, Err(RoleError::NameTaken)));
		let first = Role::create(&pool, &actor, &request("First", &[])).await?;
		Role::create(&pool, &actor, &request("Second", &[])).await?;
		assert!(matches!(
			Role::update(&pool, &actor, &first.id, &request("Second", &[])).await,
			Err(RoleError::NameTaken)
		));
		assert!(matches!(
			Role::update(&pool, &actor, &Uuid::new_v4(), &request("Nobody", &[])).await,
			Err(RoleError::NotFound)
		));
		Ok(())
	}
}
