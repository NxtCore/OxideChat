use crate::types::oauth::OAuthUserInfo;
use crate::types::user::User;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const SETUP_ADVISORY_LOCK_ID: i64 = 0x6F78_6373_6574_7570;

impl User {
	/// Creates the first admin user inside a transaction guarded by an advisory lock.
	///
	/// Returns `Ok(None)` when a user already exists, so concurrent first-time setup
	/// requests cannot create two admins.
	///
	/// # Errors
	///
	/// Returns a database error when the lock, the existence check, or any write fails.
	pub async fn create_first_admin(pool: &PgPool, email: &str, username: &str, password_hash: &str) -> Result<Option<Self>, sqlx::Error> {
		let mut tx = pool.begin().await?;
		sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(SETUP_ADVISORY_LOCK_ID).execute(&mut *tx).await?;
		let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users)").fetch_one(&mut *tx).await?;
		if exists {
			return Ok(None);
		}
		let user: Self = sqlx::query_as(
			"INSERT INTO users (email, username, password_hash, auth_method)
			 VALUES ($1, $2, $3, 'local')
			 RETURNING *",
		)
		.bind(email)
		.bind(username)
		.bind(password_hash)
		.fetch_one(&mut *tx)
		.await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = 'admin'")
			.bind(user.id)
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		Ok(Some(user))
	}

	/// Stores a new session row for a user.
	///
	/// # Errors
	///
	/// Returns a database error when the insert fails.
	pub async fn insert_session(pool: &PgPool, session_id: Uuid, user_id: &Uuid, expires_at: DateTime<Utc>) -> Result<(), sqlx::Error> {
		sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
			.bind(session_id)
			.bind(user_id)
			.bind(expires_at)
			.execute(pool)
			.await?;
		Ok(())
	}

	/// Returns the owner of a session, if the session exists.
	///
	/// # Errors
	///
	/// Returns a database error when the lookup fails.
	pub async fn session_owner(pool: &PgPool, session_id: &Uuid) -> Result<Option<Uuid>, sqlx::Error> {
		sqlx::query_scalar("SELECT user_id FROM sessions WHERE id = $1").bind(session_id).fetch_optional(pool).await
	}

	/// Deletes a session.
	///
	/// # Errors
	///
	/// Returns a database error when the delete fails.
	pub async fn delete_session(pool: &PgPool, session_id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM sessions WHERE id = $1").bind(session_id).execute(pool).await?;
		Ok(())
	}

	/// Returns the user linked to an OAuth identity, if any.
	///
	/// # Errors
	///
	/// Returns a database error when the lookup fails.
	pub async fn find_by_identity(pool: &PgPool, provider: &str, provider_user_id: &str) -> Result<Option<Self>, sqlx::Error> {
		let user_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM user_identities WHERE provider = $1 AND provider_user_id = $2")
			.bind(provider)
			.bind(provider_user_id)
			.fetch_optional(pool)
			.await?;
		match user_id {
			Some(id) => sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(id).fetch_one(pool).await.map(Some),
			None => Ok(None),
		}
	}

	/// Creates a user from an OAuth login, grants the `user` role and links the identity.
	///
	/// # Errors
	///
	/// Returns a database error when any write fails.
	pub async fn create_from_oauth(pool: &PgPool, email: &str, username: &str, info: &OAuthUserInfo) -> Result<Self, sqlx::Error> {
		let user: Self = sqlx::query_as(
			"INSERT INTO users (email, username, auth_method)
			 VALUES ($1, $2, 'oauth')
			 RETURNING *",
		)
		.bind(email)
		.bind(username)
		.fetch_one(pool)
		.await?;
		user.assign_role(pool, "user").await?;
		sqlx::query(
			"INSERT INTO user_identities (user_id, provider, provider_user_id, provider_data)
			 VALUES ($1, $2, $3, $4)",
		)
		.bind(user.id)
		.bind(&info.provider)
		.bind(&info.provider_user_id)
		.bind(&info.raw_data)
		.execute(pool)
		.await?;
		Ok(user)
	}
}
