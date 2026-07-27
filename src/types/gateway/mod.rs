mod repository;
mod responses;

pub use responses::*;

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct GatewayCredential {
	pub key_id: Uuid,
	pub project_id: Uuid,
	pub user_id: Uuid,
	pub team_id: Option<Uuid>,
	pub project_name: String,
	pub secret_hash: String,
	pub scopes: Json<Vec<String>>,
	pub key_enabled: bool,
	pub project_enabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct GatewayAuthContext {
	pub key_id: Uuid,
	pub project_id: Uuid,
	pub user_id: Uuid,
	pub team_id: Option<Uuid>,
	pub project_name: String,
	pub scopes: Vec<String>,
}

impl GatewayAuthContext {
	#[must_use]
	pub fn allows(&self, scope: &str) -> bool {
		self.scopes.iter().any(|candidate| candidate == "*" || candidate == scope)
	}
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayAuthError {
	#[error("invalid_api_key")]
	Invalid,
	#[error("gateway_auth_unavailable")]
	Unavailable,
}
