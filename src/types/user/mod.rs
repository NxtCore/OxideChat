mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

use crate::types::BaseType;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct User {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub password_hash: Option<String>,
	pub auth_method: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for User {}
