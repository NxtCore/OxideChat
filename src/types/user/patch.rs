use crate::types::Team;
use crate::types::user::User;
use sqlx::PgPool;
use uuid::Uuid;

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

	pub async fn delete(&self, pool: &PgPool) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM users WHERE id = $1").bind(self.id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

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

	pub async fn set_teams(&self, pool: &PgPool, team_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		Team::set_user_teams(pool, &self.id, team_ids).await
	}
}
