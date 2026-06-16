use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Translation {
	pub id: Uuid,
	pub language: String,
	pub key_path: String,
	pub value: String,
	pub is_override: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl BaseType for Translation {}
