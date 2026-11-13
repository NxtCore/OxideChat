use crate::types::user::User;
use crate::types::{CreateAdminUserRequest, Role, RoleError, Team};
use sqlx::PgPool;

impl User {
	pub async fn update(&mut self, pool: &PgPool, email: Option<&str>, username: Option<&str>) -> Result<(), sqlx::Error> {
		let new_email = email.unwrap_or(&self.email);
		let new_username = username.unwrap_or(&self.username);

		let updated = sqlx::query_as::<_, User>("UPDATE users SET email = $2, username = $3, updated_at = NOW() WHERE id = $1 RETURNING *")
			.bind(self.id)
			.bind(new_email)
			.bind(new_username)
			.fetch_one(pool)
			.await?;

		*self = updated;
		Ok(())
	}

	/// Deletes the user unless nobody could manage roles afterwards.
	///
	/// # Errors
	/// Returns [`RoleError::LockOut`] when the user was the last one able to manage roles, and the database error when a statement fails.
	pub async fn delete(&self, pool: &PgPool) -> Result<bool, RoleError> {
		let mut tx = pool.begin().await?;
		let held_before = Role::lock_managers(&mut tx).await?;
		let result = sqlx::query("DELETE FROM users WHERE id = $1").bind(self.id).execute(&mut *tx).await?;
		Role::check_managers_kept(&mut tx, held_before).await?;
		tx.commit().await?;
		Ok(result.rows_affected() > 0)
	}

	/// Creates a local account with its defaults, roles and teams in one transaction; nothing is stored when a step fails.
	///
	/// # Errors
	/// Returns the database error when a statement fails, for example a foreign key violation for a team that does not exist.
	pub async fn create_with_assignments(pool: &PgPool, request: &CreateAdminUserRequest, password_hash: &str) -> Result<Self, sqlx::Error> {
		let mut tx = pool.begin().await?;
		let user = Self::create(&mut *tx, &request.email, &request.username, password_hash).await?;
		user.initialize_defaults(&mut tx).await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = ANY($2)")
			.bind(user.id)
			.bind(&request.roles)
			.execute(&mut *tx)
			.await?;
		if let Some(team_ids) = &request.team_ids {
			Team::replace_user_teams(&mut tx, &user.id, team_ids).await?;
		}
		tx.commit().await?;
		Ok(user)
	}

	#[cfg(test)]
	pub async fn set_roles(&self, pool: &PgPool, role_names: &[String]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;

		sqlx::query("DELETE FROM user_roles WHERE user_id = $1").bind(self.id).execute(&mut *tx).await?;

		for role_name in role_names {
			sqlx::query(
				"INSERT INTO user_roles (user_id, role_id)
                 SELECT $1, id FROM roles WHERE name = $2
                 ON CONFLICT DO NOTHING",
			)
			.bind(self.id)
			.bind(role_name)
			.execute(&mut *tx)
			.await?;
		}

		tx.commit().await?;
		Ok(())
	}

	pub async fn assign_role(&self, pool: &PgPool, role_name: &str) -> Result<(), sqlx::Error> {
		sqlx::query(
			"INSERT INTO user_roles (user_id, role_id)
             SELECT $1, id FROM roles WHERE name = $2",
		)
		.bind(self.id)
		.bind(role_name)
		.execute(pool)
		.await?;
		Ok(())
	}

	pub async fn set_password(&self, pool: &PgPool, password_hash: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(password_hash)
			.execute(pool)
			.await?;
		Ok(())
	}
}
