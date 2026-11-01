use super::{SessionResponse, User};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow)]
struct SessionRow {
	id: Uuid,
	created_at: DateTime<Utc>,
	expires_at: DateTime<Utc>,
}

impl User {
	/// The session ID is the login cookie's value, so clients only ever see this hash of it.
	#[must_use]
	pub fn session_public_id(session_id: &Uuid) -> String {
		format!("{:x}", Sha256::digest(session_id.as_bytes()))
	}

	async fn active_sessions(pool: &PgPool, user_id: &Uuid) -> Result<Vec<SessionRow>, sqlx::Error> {
		sqlx::query_as("SELECT id, created_at, expires_at FROM sessions WHERE user_id = $1 AND expires_at > NOW() ORDER BY created_at DESC")
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	/// Lists a user's active sessions, marking the one making this request.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list_sessions(pool: &PgPool, user_id: &Uuid, current: Option<&Uuid>) -> Result<Vec<SessionResponse>, sqlx::Error> {
		let rows = Self::active_sessions(pool, user_id).await?;
		Ok(rows
			.iter()
			.map(|row| SessionResponse {
				id: Self::session_public_id(&row.id),
				created_at: row.created_at,
				expires_at: row.expires_at,
				is_current: current == Some(&row.id),
			})
			.collect())
	}

	/// Ends one of the user's own sessions. Returns false if they have no session with that public ID.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn revoke_session(pool: &PgPool, user_id: &Uuid, public_id: &str) -> Result<bool, sqlx::Error> {
		let rows = Self::active_sessions(pool, user_id).await?;
		let Some(row) = rows.iter().find(|row| Self::session_public_id(&row.id) == public_id) else {
			return Ok(false);
		};
		Self::delete_session(pool, &row.id).await?;
		Ok(true)
	}
}
