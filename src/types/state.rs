//! Shared application state.

use sqlx::PgPool;

/// Shared application state containing the database pool.
#[derive(Clone, Debug)]
pub struct JobState {
	pub db: PgPool,
}
