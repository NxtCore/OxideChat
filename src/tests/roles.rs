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
			sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type = 'role' AND target_id = $1 ORDER BY created_at")
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
			assert!(matches!(Role::update(&pool, &actor, &id, &request("Renamed", &[])).await, Err(RoleError::Protected)));
			assert!(matches!(Role::delete(&pool, &actor, &id).await, Err(RoleError::Protected)));
		}
		let user_role = builtin_id(&pool, "user").await?;
		let changed = Role::update(&pool, &actor, &user_role, &request("user", &["settings.profile.view"])).await?;
		assert!(changed.is_builtin);
		assert_eq!(changed.permissions, ["settings.profile.view"]);
		assert_eq!(audit_events(&pool, &user_role).await?, ["role_updated"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_last_role_that_can_manage_roles_keeps_that_permission(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let held = vec!["*".to_owned()];
		let actor = RoleActor {
			id: &actor_id,
			permissions: &held,
		};
		let admin_role = builtin_id(&pool, "admin").await?;
		let without_roles = request("admin", &["admin.users.edit", "settings.*"]);

		assert!(matches!(Role::update(&pool, &actor, &admin_role, &without_roles).await, Err(RoleError::LockOut)));
		let unchanged = Role::find_detailed(&pool, &admin_role).await?.ok_or("admin role")?;
		assert!(unchanged.permissions.iter().any(|permission| permission == "admin.*"));

		let managers = Role::create(&pool, &actor, &request("Managers", &["admin.roles.edit"])).await?;
		Role::update(&pool, &actor, &admin_role, &without_roles).await?;
		assert!(matches!(Role::delete(&pool, &actor, &managers.id).await, Err(RoleError::LockOut)));
		assert!(matches!(
			Role::update(&pool, &actor, &managers.id, &request("Managers", &[])).await,
			Err(RoleError::LockOut)
		));
		assert!(Role::find_detailed(&pool, &managers.id).await?.is_some());

		let wildcard = Role::create(&pool, &actor, &request("Roles admins", &["admin.*"])).await?;
		Role::delete(&pool, &actor, &managers.id).await?;
		assert!(Role::find_detailed(&pool, &wildcard.id).await?.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_role_nobody_holds_does_not_count_as_managing_roles(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let held = vec!["*".to_owned()];
		let actor = RoleActor {
			id: &actor_id,
			permissions: &held,
		};
		let admin_role = builtin_id(&pool, "admin").await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)").bind(actor_id).bind(admin_role).execute(&pool).await?;
		let unused = Role::create(&pool, &actor, &request("Unused", &["admin.roles.edit"])).await?;
		let without_roles = request("admin", &["admin.users.edit", "settings.*"]);

		assert!(matches!(Role::update(&pool, &actor, &admin_role, &without_roles).await, Err(RoleError::LockOut)));
		sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)").bind(actor_id).bind(unused.id).execute(&pool).await?;
		Role::update(&pool, &actor, &admin_role, &without_roles).await?;
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
	async fn user_admins_cannot_hand_out_or_take_over_stronger_roles(pool: PgPool) -> TestResult {
		let actor_id = create_user(&pool).await?;
		let admin_held = admin_permissions();
		let admin = RoleActor {
			id: &actor_id,
			permissions: &admin_held,
		};
		let desk_held = vec!["admin.users.edit".to_owned()];
		let desk = RoleActor {
			id: &actor_id,
			permissions: &desk_held,
		};
		Role::create(&pool, &admin, &request("Desk", &["admin.users.edit"])).await?;
		let names = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();

		assert!(matches!(desk.check_role_change(&pool, &[], &names(&["admin"])).await, Err(RoleError::PermissionNotHeld)));
		assert!(matches!(desk.check_role_change(&pool, &names(&["admin"]), &[]).await, Err(RoleError::PermissionNotHeld)));
		desk.check_role_change(&pool, &[], &names(&["Desk"])).await?;
		desk.check_role_change(&pool, &names(&["admin"]), &names(&["admin", "Desk"])).await?;

		let admin_id = create_user(&pool).await?;
		let admin_user = User::find_by_id(&pool, &admin_id).await?.ok_or("admin")?;
		admin_user.set_roles(&pool, &names(&["admin"])).await?;
		assert!(matches!(desk.check_outranks(&pool, &admin_user).await, Err(RoleError::Outranked)));
		admin.check_outranks(&pool, &admin_user).await?;

		let plain_id = create_user(&pool).await?;
		let plain = User::find_by_id(&pool, &plain_id).await?.ok_or("plain")?;
		plain.set_roles(&pool, &names(&["Desk"])).await?;
		desk.check_outranks(&pool, &plain).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn user_admins_cannot_edit_or_move_stronger_accounts(pool: PgPool) -> TestResult {
		use axum::body::Body;
		use axum::http::{Request, StatusCode};
		use tower::ServiceExt;

		crate::config::Config::init(&pool).await?;
		crate::i18n::I18n::init(&pool).await;
		let admin_held = admin_permissions();
		let setup_id = create_user(&pool).await?;
		let admin = RoleActor {
			id: &setup_id,
			permissions: &admin_held,
		};
		Role::create(&pool, &admin, &request("Desk", &["admin.users.edit"])).await?;

		let desk_id = create_user(&pool).await?;
		User::find_by_id(&pool, &desk_id).await?.ok_or("desk")?.set_roles(&pool, &["Desk".to_owned()]).await?;
		let session = Uuid::new_v4();
		User::insert_session(&pool, session, &desk_id, chrono::Utc::now() + chrono::Duration::days(1)).await?;
		let target_id = create_user(&pool).await?;
		let target = User::find_by_id(&pool, &target_id).await?.ok_or("target")?;
		target.set_roles(&pool, &["admin".to_owned()]).await?;

		let router = crate::routes::build_router(crate::tests::gateway::support::job_state(pool.clone()));
		let put = |uri: String, body: &'static str| {
			Request::builder()
				.method("PUT")
				.uri(uri)
				.header("content-type", "application/json")
				.header("cookie", format!("{}={session}", crate::utils::auth::SESSION_COOKIE_NAME))
				.body(Body::from(body))
		};
		let email = router.clone().oneshot(put(format!("/api/v1/admin/users/{target_id}"), r#"{"email":"taken@example.com"}"#)?).await?;
		assert_eq!(email.status(), StatusCode::FORBIDDEN);
		let teams = router.oneshot(put(format!("/api/v1/admin/users/{target_id}/assignments"), r#"{"targets":[]}"#)?).await?;
		assert_eq!(teams.status(), StatusCode::FORBIDDEN);
		let after = User::find_by_id(&pool, &target_id).await?.ok_or("target")?;
		assert_eq!(after.email, target.email);
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
