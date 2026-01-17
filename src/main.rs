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
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;

#[tokio::main]
async fn main() {
	dotenv::dotenv().ok();

	let db_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
	let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
	let db_name = std::env::var("POSTGRES_DB").expect("POSTGRES_DB must be set");
	let db_user = std::env::var("POSTGRES_USER").expect("POSTGRES_USER must be set");
	let db_pass = std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be set");

	let database_url = format!("postgresql://{}:{}@{}:{}/{}", db_user, db_pass, db_host, db_port, db_name);

	println!("[DATABASE] Connecting to PostgreSQL at {}:{}...", db_host, db_port);
	let pool = PgPoolOptions::new()
		.max_connections(10)
		.connect(&database_url)
		.await
		.expect("Failed to connect to database. Ensure the database exists and credentials are correct.");
	println!("[DATABASE] Connected to '{}'", db_name);

	match sqlx::migrate!("./migrations").run(&pool).await {
		Ok(_) => println!("[DATABASE] Migrations completed"),
		Err(e) => eprintln!("[DATABASE] Migration failed: {}", e),
	}

	// Initialize global configuration
	config::Config::init(&pool).await;
	println!("[CONFIG] Configuration loaded");

	// Initialize global i18n translations
	i18n::I18n::init(&pool).await;
	println!("[I18N] Translations loaded");

	// Initialize optional API key encryption
	utils::encryption::init();
	if utils::encryption::is_enabled() {
		println!("[ENCRYPTION] API key encryption enabled");
	} else {
		println!("[ENCRYPTION] API key encryption disabled (set ENCRYPTION_KEY to enable)");
	}

	// Initialize AI engine with database providers
	ai::init(&pool).await;

	let app_state = Arc::new(JobState { db: pool });

	tokio::spawn(jobs::start_job_scheduler(app_state.clone()));

	let app = Router::new()
		.merge(routes::build_router())
		.with_state(app_state)
		.layer(DefaultBodyLimit::max(8 * 1024 * 1024));
	let address = format!(
		"{}:{}",
		std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
		std::env::var("PORT").unwrap_or_else(|_| "8080".to_string())
	);
	let listener = tokio::net::TcpListener::bind(&address).await.expect("Failed to bind to address");

	println!("[SERVER] Listening on http://{}", address);
	axum::serve(listener, app).await.expect("Server error");
}
