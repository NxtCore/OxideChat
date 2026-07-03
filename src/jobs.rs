//! Background job scheduler for OxideChat.
//!
//! Contains scheduled tasks like session cleanup and provider model sync that run periodically.

use crate::types::providers::Provider;
use crate::utils::providers::sync_provider_models;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

/// Minimum interval between session cleanup runs.
const SESSION_CLEANUP_INTERVAL_SECS: u64 = 3600; // 1 hour
const PROVIDER_SYNC_INTERVAL_SECS: u64 = 500; // 5 minutes

/// Start the background job scheduler.
///
/// Spawns independent tokio tasks for each scheduled job.
pub async fn start_job_scheduler(state: Arc<super::JobState>) {
	println!("[JOBS] Starting job scheduler");

	let handles: Vec<tokio::task::JoinHandle<()>> = vec![
		tokio::spawn(session_cleanup_job(Arc::clone(&state))),
		tokio::spawn(provider_sync_job(Arc::clone(&state))),
	];

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

/// Periodically sync models for all enabled system providers.
///
/// Runs every `PROVIDER_SYNC_INTERVAL_SECS` so newly released models
/// (e.g. a new Claude Sonnet) become available without a manual sync.
async fn provider_sync_job(state: Arc<super::JobState>) {
	println!("[JOBS] Provider model sync job started");

	loop {
		match Provider::list_enabled_system(&state.db).await {
			Ok(providers) => {
				for provider in providers {
					match sync_provider_models(&state.db, &provider).await {
						Ok(summary) => {
							println!(
								"[JOBS] Synced provider '{}': +{}/~{}/-{} models",
								provider.name, summary.models_added, summary.models_updated, summary.models_removed
							);
						}
						Err(e) => {
							eprintln!("[JOBS] Failed to sync provider '{}': {e}", provider.name);
						}
					}
				}
			}
			Err(e) => {
				eprintln!("[JOBS] Failed to list providers for sync: {e}");
			}
		}
		tokio::time::sleep(Duration::from_secs(PROVIDER_SYNC_INTERVAL_SECS)).await;
	}
}
