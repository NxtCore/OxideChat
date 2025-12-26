use std::sync::Arc;

pub async fn start_job_scheduler(_state: Arc<super::AppState>) {
	println!("[JOBS] Starting job scheduler");

	let handles: Vec<tokio::task::JoinHandle<()>> = vec![
		// tokio::spawn(health_check_job(Arc::clone(&state))),
		// tokio::spawn(maintenance_job(Arc::clone(&state))),
	];

	for handle in handles {
		let _ = handle.await;
	}
}
