mod repository;
mod requests;
mod responses;

pub use requests::*;
pub use responses::*;

use crate::types::BaseType;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct Team {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub is_default: bool,
	pub allow_all_models: bool,
	pub budget_id: Option<Uuid>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl BaseType for Team {}
