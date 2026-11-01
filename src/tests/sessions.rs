#[cfg(test)]
mod tests {
	use crate::tests::support::{TestResult, create_user};
	use crate::types::User;
	use crate::types::consts::{SETTINGS_APPEARANCE_EDIT, SETTINGS_SESSIONS_REVOKE, SETTINGS_SESSIONS_VIEW};
	use chrono::{Duration, Utc};
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn add_session(pool: &PgPool, user_id: &Uuid, expires_in_days: i64) -> TestResult<Uuid> {
		let id = Uuid::new_v4();
		User::insert_session(pool, id, user_id, Utc::now() + Duration::days(expires_in_days)).await?;
		Ok(id)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_hides_real_ids_and_skips_expired_sessions(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let current = add_session(&pool, &user, 7).await?;
		let other = add_session(&pool, &user, 7).await?;
		add_session(&pool, &user, -1).await?;

		let sessions = User::list_sessions(&pool, &user, Some(&current)).await?;

		assert_eq!(sessions.len(), 2);
		assert!(sessions.iter().all(|session| session.id != current.to_string() && session.id != other.to_string()));
		let marked: Vec<&str> = sessions.iter().filter(|session| session.is_current).map(|session| session.id.as_str()).collect();
		assert_eq!(marked, [User::session_public_id(&current).as_str()]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn revoke_ends_only_the_owners_session(pool: PgPool) -> TestResult {
		let owner = create_user(&pool).await?;
		let stranger = create_user(&pool).await?;
		let session = add_session(&pool, &owner, 7).await?;
		let public_id = User::session_public_id(&session);

		assert!(!User::revoke_session(&pool, &stranger, &public_id).await?);
		assert!(User::find_by_session(&pool, &session).await?.is_some());
		assert!(!User::revoke_session(&pool, &owner, &session.to_string()).await?);
		assert!(User::find_by_session(&pool, &session).await?.is_some());

		assert!(User::revoke_session(&pool, &owner, &public_id).await?);
		assert!(User::find_by_session(&pool, &session).await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn members_hold_the_personal_settings_permissions(pool: PgPool) -> TestResult {
		let id = create_user(&pool).await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = 'user'").bind(id).execute(&pool).await?;
		let member = User::find_by_id(&pool, &id).await?.ok_or("user")?;
		for permission in [SETTINGS_APPEARANCE_EDIT, SETTINGS_SESSIONS_VIEW, SETTINGS_SESSIONS_REVOKE] {
			assert!(member.check_permission(&pool, permission).await?, "{permission}");
		}
		Ok(())
	}
}
