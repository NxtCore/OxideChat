use serde::Deserialize;

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
