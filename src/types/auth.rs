use serde::Deserialize;
use sqlx::FromRow;

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
	pub email: String,
	pub username: String,
	pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
	pub email: String,
	pub username: String,
	pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
	pub email: String,
	pub password: String,
}

#[derive(Debug, FromRow)]
pub struct CountRow {
	pub count: i64,
}
