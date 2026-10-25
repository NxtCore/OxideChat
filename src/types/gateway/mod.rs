mod body;
mod estimate;
mod keys;
mod keys_repository;
mod limiter;
mod policy;
mod repository;

pub(crate) use body::ReservedBody;
pub use keys::*;
pub use limiter::*;
pub use policy::*;

use crate::types::{BudgetAdmission, BudgetError, BudgetSubject, RequestLogContext, RequestLogOwner, RequestLogTrace};
use axum::body::Body;
use axum::http::HeaderValue;
use axum::response::Response;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use omniference::types::providers::OpenAIModel;
use sqlx::types::Json;
use uuid::Uuid;

/// Scope required to list models through the inference gateway.
pub const INFERENCE_READ_SCOPE: &str = "inference:read";
/// Scope required to run inference through the gateway.
pub const INFERENCE_WRITE_SCOPE: &str = "inference:write";
/// Request metadata key carrying the limiter reservation id to usage reporting.
pub const RESERVATION_METADATA_KEY: &str = "oxide_reservation_id";
/// Request metadata key carrying the budget reservation id to usage reporting.
pub const BUDGET_RESERVATION_METADATA_KEY: &str = "oxide_budget_reservation_id";
/// Request metadata key carrying the public gateway model id to usage reporting.
pub const PUBLIC_MODEL_METADATA_KEY: &str = "oxide_public_model_id";
/// Response header set when a warning budget is exhausted.
pub const BUDGET_WARNING_HEADER: &str = "x-oxide-budget-warning";
/// Response header naming the authenticated API key.
pub const API_KEY_ID_HEADER: &str = "x-oxide-api-key-id";
/// Response header naming the authenticated API key's team.
pub const TEAM_ID_HEADER: &str = "x-oxide-team-id";
/// Response header carrying the concurrency limit.
pub const CONCURRENT_LIMIT_HEADER: &str = "x-oxide-concurrent-limit";
/// Response header carrying the remaining concurrency.
pub const CONCURRENT_REMAINING_HEADER: &str = "x-oxide-concurrent-remaining";

// Response Types

/// Enabled provider model exposed through the OpenAI-compatible gateway.
#[derive(Debug, sqlx::FromRow)]
pub struct GatewayModel {
	pub provider_name: String,
	pub model_id: String,
	pub created_at: DateTime<Utc>,
}

/// Public gateway id of a stored model.
///
/// Discovered models already carry their provider prefix, so the stored id is
/// returned unchanged; ids without the prefix get it added.
#[must_use]
pub fn public_model_id(provider_name: &str, model_id: &str) -> String {
	let provider = provider_name.to_ascii_lowercase();
	match model_id.split_once('/') {
		Some((prefix, _)) if prefix.eq_ignore_ascii_case(&provider) => model_id.to_string(),
		_ => format!("{provider}/{model_id}"),
	}
}

impl From<GatewayModel> for OpenAIModel {
	fn from(model: GatewayModel) -> Self {
		Self {
			id: public_model_id(&model.provider_name, &model.model_id),
			object: Some("model".to_string()),
			created: Some(model.created_at.timestamp().max(0) as u64),
			owned_by: Some(model.provider_name),
		}
	}
}

// Internal Types

/// Stored gateway API-key and team data used during authentication.
#[derive(Debug, sqlx::FromRow)]
pub struct GatewayCredential {
	pub key_id: Uuid,
	pub user_id: Uuid,
	pub team_id: Uuid,
	pub secret_hash: String,
	pub scopes: Json<Vec<String>>,
	pub key_enabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub is_member: bool,
	pub team_rules: Json<Vec<GatewayRateRule>>,
	pub key_rules: Json<Vec<GatewayRateRule>>,
	pub team_concurrency: Option<i32>,
	pub key_concurrency: Option<i32>,
}

/// Authenticated identity and permissions associated with a gateway request.
#[derive(Clone, Debug)]
pub struct GatewayAuthContext {
	pub key_id: Uuid,
	pub user_id: Uuid,
	pub team_id: Uuid,
	pub scopes: Vec<String>,
	pub policy: Arc<GatewayRatePolicy>,
}

impl GatewayAuthContext {
	/// Returns whether this API key grants the requested scope.
	#[must_use]
	pub fn allows(&self, scope: &str) -> bool {
		self.scopes.iter().any(|candidate| candidate == "*" || candidate == scope)
	}

	#[must_use]
	/// The account request log rows for this key belong to.
	pub fn owner(&self) -> RequestLogOwner {
		RequestLogOwner {
			user_id: self.user_id,
			team_id: self.team_id,
			api_key_id: self.key_id,
		}
	}

	#[must_use]
	/// The identity whose budgets apply to this key's requests.
	pub fn budget_subject(&self) -> BudgetSubject {
		BudgetSubject::user_key(self.user_id, self.key_id)
	}

	#[must_use]
	/// Builds the admission request for one route and token reservation.
	pub fn admission_request(&self, route: GatewayRoute, tokens: u64) -> GatewayAdmissionRequest {
		GatewayAdmissionRequest {
			team_id: self.team_id,
			key_id: self.key_id,
			policy: Arc::clone(&self.policy),
			route_class: route.class,
			tokens,
		}
	}
}

/// Model authorized for gateway inference.
#[derive(Debug)]
pub struct GatewayInference {
	pub model_id: Uuid,
	pub engine_model_id: String,
	pub public_model_id: String,
	pub provider_name: String,
	pub max_output_tokens: Option<u32>,
	pub context_length: Option<u32>,
	pub budget: BudgetAdmission,
}

impl GatewayInference {
	#[must_use]
	/// Id of the budget reservation to report usage against, if one was made.
	pub fn budget_reservation_id(&self) -> Option<Uuid> {
		self.budget.reservation.as_ref().map(|reservation| reservation.id)
	}

	#[must_use]
	/// Adds the budget warning header and ties the budget reservation to the response body.
	pub fn finish(self, mut response: Response) -> Response {
		if self.budget.warn {
			response.headers_mut().insert(BUDGET_WARNING_HEADER, HeaderValue::from_static("exceeded"));
		}
		match self.budget.reservation {
			Some(reservation) => {
				let (parts, body) = response.into_parts();
				Response::from_parts(parts, Body::new(ReservedBody::new(body, reservation)))
			}
			None => response,
		}
	}
}

/// Request-scoped inputs shared by every outcome of one gateway inference request.
#[derive(Clone, Copy)]
pub struct GatewayRequestScope<'a> {
	pub db: &'a sqlx::PgPool,
	pub context: &'a GatewayAuthContext,
	pub log: RequestLogContext,
	pub usage_type: &'static str,
	pub streaming: bool,
}

impl GatewayRequestScope<'_> {
	#[must_use]
	/// Describes this request for its log row, naming the model when it is known.
	pub fn trace<'b>(&self, public_model_id: &'b str, model_id: Option<Uuid>, provider: Option<&'b str>) -> RequestLogTrace<'b> {
		RequestLogTrace {
			context: self.log,
			owner: self.context.owner(),
			request_type: self.usage_type,
			public_model_id,
			model_id,
			provider,
			streaming: self.streaming,
		}
	}

	#[must_use]
	/// Describes this request for its log row using a resolved model.
	pub fn inference_trace<'b>(&self, inference: &'b GatewayInference) -> RequestLogTrace<'b> {
		self.trace(&inference.public_model_id, Some(inference.model_id), Some(&inference.provider_name))
	}
}

/// Shared inputs for one gateway inference run.
pub struct GatewayRun<'a> {
	pub scope: GatewayRequestScope<'a>,
	pub reservation_id: Uuid,
}

/// Failures produced while authenticating a gateway credential.
#[derive(Debug, thiserror::Error)]
pub enum GatewayAuthError {
	/// The token, key state, team membership, or expiry is invalid.
	#[error("invalid_api_key")]
	Invalid,
	/// Credential storage or password verification is unavailable.
	#[error("gateway_auth_unavailable")]
	Unavailable,
}

/// An authentication failure and, when the key's secret verified, the key's owner.
#[derive(Debug)]
pub struct GatewayAuthFailure {
	pub error: GatewayAuthError,
	pub owner: Option<RequestLogOwner>,
}

impl From<GatewayAuthError> for GatewayAuthFailure {
	fn from(error: GatewayAuthError) -> Self {
		Self { error, owner: None }
	}
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
	/// Budget enforcement could not run.
	#[error("gateway_budget_unavailable")]
	BudgetUnavailable,
	/// Model or budget policy storage could not be queried.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

impl From<BudgetError> for GatewayModelAccessError {
	fn from(error: BudgetError) -> Self {
		match error {
			BudgetError::Exceeded => Self::BudgetExceeded,
			BudgetError::Unavailable => Self::BudgetUnavailable,
			BudgetError::Database(error) => Self::Database(error),
		}
	}
}
