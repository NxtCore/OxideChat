use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

mod repository;
mod rows;

#[derive(Debug, Serialize, FromRow)]
pub struct Role {
	pub id: Uuid,
	pub name: String,
	pub created_at: DateTime<Utc>,
}

impl BaseType for Role {}
