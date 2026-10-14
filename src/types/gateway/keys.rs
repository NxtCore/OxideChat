use super::{GatewayRateRule, GatewayRouteClass, GatewayRuleMetric, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, types::Json};
use uuid::Uuid;

/// Longest allowed rotation overlap in seconds.
pub const MAX_ROTATION_OVERLAP_SECONDS: i64 = 7 * 24 * 60 * 60;
/// Rotation overlap used when the request does not name one.
pub const DEFAULT_ROTATION_OVERLAP_SECONDS: i64 = 24 * 60 * 60;
/// Longest allowed API key name.
pub const MAX_KEY_NAME_LENGTH: usize = 100;
/// Scopes a key can be granted.
pub const KEY_SCOPES: [&str; 2] = [INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE];

fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
	Option::<T>::deserialize(deserializer).map(Some)
}

// Request Types

#[derive(Debug, Deserialize)]
/// Body of a key creation request.
pub struct CreateGatewayKeyRequest {
	pub name: String,
	pub team_id: Uuid,
	pub scopes: Vec<String>,
	pub expires_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
/// Body of a key update request; `null` clears expiry or concurrency.
pub struct UpdateGatewayKeyRequest {
	pub name: Option<String>,
	pub is_enabled: Option<bool>,
	pub scopes: Option<Vec<String>>,
	#[serde(default, deserialize_with = "nullable")]
	pub expires_at: Option<Option<DateTime<Utc>>>,
	#[serde(default, deserialize_with = "nullable")]
	pub max_concurrent_requests: Option<Option<i32>>,
}

#[derive(Debug, Default, Deserialize)]
/// Body of a key rotation request.
pub struct RotateGatewayKeyRequest {
	pub overlap_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
/// One key-scoped rate rule submitted by a client.
pub struct GatewayKeyRuleRequest {
	pub route_class: GatewayRouteClass,
	pub metric: GatewayRuleMetric,
	pub capacity: u64,
	pub refill_period_seconds: u32,
}

#[derive(Debug, Deserialize)]
/// Body replacing all key-scoped rate rules.
pub struct SetGatewayKeyLimitsRequest {
	pub rules: Vec<GatewayKeyRuleRequest>,
}

// Response Types

#[derive(Debug, Serialize)]
/// A key as shown to its owner; never includes the secret or its hash.
pub struct GatewayKeyResponse {
	pub id: Uuid,
	pub name: String,
	pub team_id: Uuid,
	pub key_prefix: String,
	pub last_four: String,
	pub scopes: Vec<String>,
	pub is_enabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub last_used_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// A newly issued key including its one-time token.
pub struct GatewayKeyTokenResponse {
	#[serde(flatten)]
	pub key: GatewayKeyResponse,
	pub token: String,
}

#[derive(Debug, Serialize)]
/// Result of a rotation: the new key and the old key with its overlap expiry.
pub struct GatewayKeyRotationResponse {
	pub key: GatewayKeyTokenResponse,
	pub previous: GatewayKeyResponse,
}

#[derive(Debug, Serialize)]
/// One stored key-scoped rate rule.
pub struct GatewayKeyRuleResponse {
	pub id: Uuid,
	pub route_class: String,
	pub metric: String,
	pub capacity: i64,
	pub refill_period_seconds: i32,
}

#[derive(Debug, Serialize)]
/// A team the caller belongs to and the models it grants.
pub struct GatewayTeamResponse {
	pub id: Uuid,
	pub name: String,
	pub is_default: bool,
	pub models: Vec<String>,
}

// Internal Types

#[derive(Debug, Clone, FromRow)]
/// Stored gateway API key row.
pub struct GatewayApiKey {
	pub id: Uuid,
	pub name: String,
	pub team_id: Uuid,
	pub key_prefix: String,
	pub last_four: String,
	pub scopes: Json<Vec<String>>,
	pub is_enabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub last_used_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
	pub created_at: DateTime<Utc>,
}

impl GatewayApiKey {
	#[must_use]
	/// Builds the owner-facing representation without any secret material.
	pub fn to_response(&self) -> GatewayKeyResponse {
		GatewayKeyResponse {
			id: self.id,
			name: self.name.clone(),
			team_id: self.team_id,
			key_prefix: self.key_prefix.clone(),
			last_four: self.last_four.clone(),
			scopes: self.scopes.0.clone(),
			is_enabled: self.is_enabled,
			expires_at: self.expires_at,
			revoked_at: self.revoked_at,
			last_used_at: self.last_used_at,
			max_concurrent_requests: self.max_concurrent_requests,
			created_at: self.created_at,
		}
	}
}

#[derive(Debug, FromRow)]
pub(super) struct GatewayKeyRuleRow {
	pub id: Uuid,
	pub route_class: String,
	pub metric: String,
	pub capacity: i64,
	pub refill_period_seconds: i32,
}

impl From<GatewayKeyRuleRow> for GatewayKeyRuleResponse {
	fn from(row: GatewayKeyRuleRow) -> Self {
		Self {
			id: row.id,
			route_class: row.route_class,
			metric: row.metric,
			capacity: row.capacity,
			refill_period_seconds: row.refill_period_seconds,
		}
	}
}

#[derive(Debug, FromRow)]
pub(super) struct GatewayTeamModelRow {
	pub team_id: Uuid,
	pub provider_name: String,
	pub model_id: String,
}

impl GatewayKeyRuleRequest {
	pub(super) fn to_rule(&self) -> GatewayRateRule {
		GatewayRateRule {
			id: Uuid::new_v4(),
			route_class: self.route_class,
			metric: self.metric,
			capacity: self.capacity,
			refill_period_seconds: self.refill_period_seconds,
		}
	}
}

/// Failures produced by key management operations.
#[derive(Debug, thiserror::Error)]
pub enum GatewayKeyError {
	/// The key does not exist or belongs to someone else.
	#[error("gateway_key_not_found")]
	NotFound,
	/// The key is revoked and can no longer change.
	#[error("gateway_key_revoked")]
	Revoked,
	/// The caller is not a member of the requested team.
	#[error("gateway_key_team_forbidden")]
	TeamForbidden,
	/// The name is empty or too long.
	#[error("gateway_key_name_invalid")]
	InvalidName,
	/// The scope list is empty or has unsupported entries.
	#[error("gateway_key_scopes_invalid")]
	InvalidScopes,
	/// The expiry is not in the future.
	#[error("gateway_key_expiry_invalid")]
	InvalidExpiry,
	/// The concurrency cap is not positive.
	#[error("gateway_key_concurrency_invalid")]
	InvalidConcurrency,
	/// The rotation overlap is outside the allowed range.
	#[error("gateway_key_overlap_invalid")]
	InvalidOverlap,
	/// A rate rule cannot be enforced.
	#[error("gateway_key_rule_invalid")]
	InvalidRule,
	/// Two rules share a route class, metric and window.
	#[error("gateway_key_rule_duplicate")]
	DuplicateRule,
	/// The secret could not be generated or hashed.
	#[error("gateway_key_secret_failed")]
	Secret,
	/// Storage failed.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}
