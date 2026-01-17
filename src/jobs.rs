//! Background job scheduler for OxideChat.
//!
//! Contains scheduled tasks like session cleanup that run periodically.

use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

/// Minimum interval between session cleanup runs.
const SESSION_CLEANUP_INTERVAL_SECS: u64 = 3600; // 1 hour

/// Start the background job scheduler.
///
/// Spawns independent tokio tasks for each scheduled job.
pub async fn start_job_scheduler(state: Arc<super::JobState>) {
	println!("[JOBS] Starting job scheduler");

	let handles: Vec<tokio::task::JoinHandle<()>> = vec![tokio::spawn(session_cleanup_job(Arc::clone(&state)))];

	for handle in handles {
		let _ = handle.await;
	}
}

/// Periodically delete expired sessions from the database.
///
/// Runs every `SESSION_CLEANUP_INTERVAL_SECS` and removes all sessions
/// where `expires_at < NOW()`. This prevents the sessions table from
/// accumulating stale entries over time.
async fn session_cleanup_job(state: Arc<super::JobState>) {
	println!("[JOBS] Session cleanup job started");

	loop {
		match cleanup_expired_sessions(&state.db).await {
			Ok(count) => {
				if count > 0 {
					println!("[JOBS] Cleaned up {count} expired sessions");
				}
			}
			Err(e) => {
				eprintln!("[JOBS] Session cleanup error: {e}");
			}
		}
		tokio::time::sleep(Duration::from_secs(SESSION_CLEANUP_INTERVAL_SECS)).await;
	}
}

/// Delete all expired sessions from the database.
///
/// Returns the number of sessions deleted.
async fn cleanup_expired_sessions(pool: &PgPool) -> Result<u64, sqlx::Error> {
	let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()").execute(pool).await?;

	Ok(result.rows_affected())
}
