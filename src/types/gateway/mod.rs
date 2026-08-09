mod repository;

use chrono::{DateTime, Utc};
use omniference::types::providers::OpenAIModel;
use sqlx::types::Json;
use uuid::Uuid;

/// Scope required to list models through the inference gateway.
pub const INFERENCE_READ_SCOPE: &str = "inference:read";
/// Scope required to run inference through the gateway.
pub const INFERENCE_WRITE_SCOPE: &str = "inference:write";

// Response Types

/// Enabled provider model exposed through the OpenAI-compatible gateway.
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

// Internal Types

/// Stored gateway API-key and project data used during authentication.
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

/// Authenticated identity and permissions associated with a gateway request.
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
	/// Returns whether this API key grants the requested scope.
	#[must_use]
	pub fn allows(&self, scope: &str) -> bool {
		self.scopes.iter().any(|candidate| candidate == "*" || candidate == scope)
	}
}

/// Model authorized for gateway inference.
#[derive(Debug)]
pub struct GatewayInference {
	pub model_id: Uuid,
}

/// Failures produced while authenticating a gateway credential.
#[derive(Debug, thiserror::Error)]
pub enum GatewayAuthError {
	/// The token, key state, project state, or expiry is invalid.
	#[error("invalid_api_key")]
	Invalid,
	/// Credential storage or password verification is unavailable.
	#[error("gateway_auth_unavailable")]
	Unavailable,
}

/// Failures produced while authorizing a model for gateway inference.
#[derive(Debug, thiserror::Error)]
pub enum GatewayModelAccessError {
	/// No unique accessible provider model matched the requested identifier.
	#[error("gateway_model_not_found")]
	NotFound,
	/// A blocking budget has reached its configured amount.
	#[error("gateway_budget_exceeded")]
	BudgetExceeded,
	/// Model or budget policy storage could not be queried.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}
