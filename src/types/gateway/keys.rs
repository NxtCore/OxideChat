use super::{GatewayRateRule, GatewayRouteClass, GatewayRuleMetric, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE};
use chrono::{DateTime, Utc};
use crate::types::EffectiveBudgetResponse;
use rust_decimal::Decimal;
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
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "CreateKeyPayload", optional_fields = nullable))]
pub struct CreateGatewayKeyRequest {
	pub name: String,
	pub team_id: Uuid,
	pub scopes: Vec<String>,
	pub expires_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
	#[serde(default)]
	pub service_account: bool,
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

#[derive(Debug, Deserialize)]
/// Body replacing the key's spending budget.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "SetKeyBudgetPayload", optional_fields = nullable))]
pub struct SetGatewayKeyBudgetRequest {
	#[cfg_attr(test, ts(type = "string"))]
	pub amount: Decimal,
	pub interval: String,
	pub on_exceed: String,
}

// Response Types

#[derive(Debug, Serialize)]
/// A key as shown to its owner; never includes the secret or its hash.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperKey"))]
pub struct GatewayKeyResponse {
	pub id: Uuid,
	pub name: String,
	pub team_id: Uuid,
	pub user_id: Option<Uuid>,
	pub service_account: bool,
	pub key_prefix: String,
	pub last_four: String,
	pub scopes: Vec<String>,
	pub is_enabled: bool,
	pub admin_disabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub last_used_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// A key of a team the caller administers, with its owner and team names.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperTeamKey"))]
pub struct GatewayTeamKeyResponse {
	#[serde(flatten)]
	pub key: GatewayKeyResponse,
	pub team_name: String,
	pub owner_username: Option<String>,
}

#[derive(Debug, Serialize)]
/// A team the caller can issue service-account keys for.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperManagedTeam"))]
pub struct GatewayManagedTeamResponse {
	pub id: Uuid,
	pub name: String,
}

#[derive(Debug, Serialize)]
/// The teams the caller administers and every key pinned to them.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperTeamKeys"))]
pub struct GatewayTeamKeysResponse {
	pub teams: Vec<GatewayManagedTeamResponse>,
	pub keys: Vec<GatewayTeamKeyResponse>,
}

#[derive(Debug, Serialize)]
/// A newly issued key including its one-time token.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperKeyWithToken"))]
pub struct GatewayKeyTokenResponse {
	#[serde(flatten)]
	pub key: GatewayKeyResponse,
	pub token: String,
}

#[derive(Debug, Serialize)]
/// Result of a rotation: the new key and the old key with its overlap expiry.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperKeyRotation"))]
pub struct GatewayKeyRotationResponse {
	pub key: GatewayKeyTokenResponse,
	pub previous: GatewayKeyResponse,
}

#[derive(Debug, Serialize)]
/// One stored key-scoped rate rule.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperKeyRule"))]
pub struct GatewayKeyRuleResponse {
	pub id: Uuid,
	pub route_class: String,
	pub metric: String,
	pub capacity: i64,
	pub refill_period_seconds: i32,
}

#[derive(Debug, Serialize)]
/// A key's spending budget with its current spend.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperKeyBudget"))]
pub struct GatewayKeyBudgetResponse {
	#[cfg_attr(test, ts(type = "string"))]
	pub amount: Decimal,
	pub interval: String,
	pub on_exceed: String,
	#[cfg_attr(test, ts(type = "string"))]
	pub spent: Decimal,
	#[cfg_attr(test, ts(type = "string"))]
	pub remaining: Decimal,
	pub window_start: DateTime<Utc>,
	pub resets_at: Option<DateTime<Utc>>,
	pub exhausted: bool,
}

impl From<EffectiveBudgetResponse> for GatewayKeyBudgetResponse {
	fn from(budget: EffectiveBudgetResponse) -> Self {
		Self {
			amount: budget.amount,
			interval: budget.budget.interval,
			on_exceed: budget.on_exceed,
			spent: budget.spent,
			remaining: budget.remaining,
			window_start: budget.window_start,
			resets_at: budget.resets_at,
			exhausted: budget.exhausted,
		}
	}
}

#[derive(Debug, Serialize)]
/// A team the caller belongs to and the models it grants.
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeveloperTeam"))]
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
	pub user_id: Option<Uuid>,
	pub key_prefix: String,
	pub last_four: String,
	pub scopes: Json<Vec<String>>,
	pub is_enabled: bool,
	pub admin_disabled: bool,
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
			user_id: self.user_id,
			service_account: self.user_id.is_none(),
			key_prefix: self.key_prefix.clone(),
			last_four: self.last_four.clone(),
			scopes: self.scopes.0.clone(),
			is_enabled: self.is_enabled,
			admin_disabled: self.admin_disabled,
			expires_at: self.expires_at,
			revoked_at: self.revoked_at,
			last_used_at: self.last_used_at,
			max_concurrent_requests: self.max_concurrent_requests,
			created_at: self.created_at,
		}
	}
}

#[derive(Debug, FromRow)]
pub(super) struct GatewayTeamKeyRow {
	#[sqlx(flatten)]
	pub key: GatewayApiKey,
	pub team_name: String,
	pub owner_username: Option<String>,
}

impl GatewayTeamKeyRow {
	pub(super) fn to_response(&self) -> GatewayTeamKeyResponse {
		GatewayTeamKeyResponse {
			key: self.key.to_response(),
			team_name: self.team_name.clone(),
			owner_username: self.owner_username.clone(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
/// The user managing keys and whether they act as an installation administrator.
pub struct KeyActor {
	pub user_id: Uuid,
	pub installation_admin: bool,
}

impl KeyActor {
	#[must_use]
	/// Builds an actor from a user id and their key administration permission.
	pub fn new(user_id: &Uuid, installation_admin: bool) -> Self {
		Self {
			user_id: *user_id,
			installation_admin,
		}
	}
}

impl From<&Uuid> for KeyActor {
	fn from(user_id: &Uuid) -> Self {
		Self::new(user_id, false)
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
	/// The caller is not allowed to manage service-account keys of the team.
	#[error("gateway_key_service_account_forbidden")]
	ServiceAccountForbidden,
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
	/// The budget amount, interval or action is unsupported.
	#[error("gateway_key_budget_invalid")]
	InvalidBudget,
	/// An administrator disabled the key and only an administrator can enable it.
	#[error("gateway_key_admin_disabled")]
	AdminDisabled,
	/// The secret could not be generated or hashed.
	#[error("gateway_key_secret_failed")]
	Secret,
	/// Storage failed.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}
