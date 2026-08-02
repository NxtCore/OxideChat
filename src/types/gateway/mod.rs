mod repository;

use chrono::{DateTime, Utc};
use omniference::types::providers::OpenAIModel;
use sqlx::types::Json;
use uuid::Uuid;

pub const INFERENCE_READ_SCOPE: &str = "inference:read";
pub const INFERENCE_WRITE_SCOPE: &str = "inference:write";

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

#[derive(Debug, sqlx::FromRow)]
pub struct GatewayModel {
	pub provider_name: String,
	pub model_id: String,
	pub created_at: DateTime<Utc>,
}

impl From<GatewayModel> for OpenAIModel {
	fn from(model: GatewayModel) -> Self {
		Self {
			id: format!("{}/{}", model.provider_name.to_ascii_lowercase(), model.model_id),
			object: Some("model".to_string()),
			created: Some(model.created_at.timestamp().max(0) as u64),
			owned_by: Some(model.provider_name),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayAuthError {
	#[error("invalid_api_key")]
	Invalid,
	#[error("gateway_auth_unavailable")]
	Unavailable,
}
