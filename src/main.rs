extern crate core;

mod ai;
mod config;
mod i18n;
mod jobs;
mod logging;
mod routes;
mod tests;
mod types;
mod utils;

use crate::types::JobState;
use axum::{Router, extract::DefaultBodyLimit};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() {
	dotenv::dotenv().ok();

	let log_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,omniference=debug"));
	tracing_subscriber::fmt().with_env_filter(log_filter).with_target(true).init();

	let db_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
	let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
	let db_name = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be set");
	let db_user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be set");
	let db_pass = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be set");

	let database_url = format!("postgresql://{}:{}@{}:{}/{}?application_name=OxideChat", db_user, db_pass, db_host, db_port, db_name);

	println!("[DATABASE] Connecting to PostgreSQL at {}:{}...", db_host, db_port);

	let pool = PgPoolOptions::new()
		.max_connections(10)
		.acquire_timeout(std::time::Duration::from_secs(10))
		.idle_timeout(std::time::Duration::from_secs(300))
		.max_lifetime(std::time::Duration::from_secs(1800))
		.connect(&database_url)
		.await
		.expect("Failed to connect to database. Ensure the database exists and credentials are correct.");

	println!("[DATABASE] Connected to '{}'", db_name);

	println!("[DATABASE] Running migrations...");
	match tokio::time::timeout(std::time::Duration::from_secs(30), sqlx::migrate!("./migrations").run(&pool)).await {
		Ok(Ok(_)) => println!("[DATABASE] Migrations completed"),
		Ok(Err(e)) => {
			eprintln!("[DATABASE] Migration failed: {}", e);
			std::process::exit(1);
		}
		Err(_) => {
			eprintln!("[DATABASE] Migration timed out after 30s");
			std::process::exit(1);
		}
	}

	match utils::encryption::init() {
		Ok(source) => println!("[ENCRYPTION] Credential encryption enabled using {}", source.as_str()),
		Err(error) => {
			eprintln!("[ENCRYPTION] Failed to initialize credential encryption: {error}");
			std::process::exit(1);
		}
	}
	config::Config::init(&pool).await;
	println!("[CONFIG] Configuration loaded");

	i18n::I18n::init(&pool).await;
	println!("[I18N] Translations loaded");

	let app_state = Arc::new(JobState {
		db: pool,
		mcp_pool: crate::utils::tools::McpConnectionPool::new(),
		client_tool_pending: crate::types::state::ClientToolPending::new(),
	});
	ai::init(&app_state).await;

	tokio::spawn(jobs::start_job_scheduler(app_state.clone()));

	let app = Router::new()
		.merge(routes::build_router(Arc::clone(&app_state)))
		.layer(DefaultBodyLimit::max(8 * 1024 * 1024));

	let address = format!(
		"{}:{}",
		std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
		std::env::var("PORT").unwrap_or_else(|_| "8080".to_string())
	);

	let listener = tokio::net::TcpListener::bind(&address).await.expect("Failed to bind to address");

	println!("[SERVER] Listening on http://{}", address);

	let server = axum::serve(listener, app).with_graceful_shutdown(async {
		shutdown_signal().await;
		println!("[SERVER] Shutdown signal received");
	});

	if let Err(e) = server.await {
		eprintln!("[SERVER] Error: {}", e);
	}

	println!("[AI] Draining usage queue...");
	ai::shutdown().await;
	println!("[DATABASE] Closing pool...");
	app_state.db.close().await;
	println!("[DATABASE] Pool closed");
}

#[cfg(unix)]
async fn shutdown_signal() {
	use tokio::signal::unix::{SignalKind, signal};

	let mut terminate = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {},
		_ = terminate.recv() => {},
	}
}

#[cfg(not(unix))]
async fn shutdown_signal() {
	tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
}
