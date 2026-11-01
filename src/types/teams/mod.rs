mod repository;
mod requests;
mod responses;

pub use requests::*;
pub use responses::*;

use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum TeamRole {
	Member,
	Admin,
}

impl TeamRole {
	#[must_use]
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Member => "member",
			Self::Admin => "admin",
		}
	}
}

#[derive(Debug, FromRow)]
pub struct Team {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub is_default: bool,
	pub allow_all_models: bool,
	pub budget_id: Option<Uuid>,
	pub default_model_id: Option<Uuid>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl BaseType for Team {}
