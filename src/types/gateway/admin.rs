use super::{GatewayKeyError, GatewayKeyRuleRequest, GatewayKeyRuleResponse};
use crate::types::{EffectiveBudgetResponse, RequestLogErrorCategory, RequestLogFilter, RequestLogStatus, RequestLogSummary};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{FromRow, Postgres, Transaction, types::Json};
use uuid::Uuid;

/// Longest reason an administrator can attach to an action.
pub const MAX_ADMIN_REASON_LENGTH: usize = 500;
/// Days ahead that expiry events look when the query names none.
pub const DEFAULT_EVENT_WINDOW_DAYS: i64 = 7;
/// Furthest window in days that expiry events can look ahead.
pub const MAX_EVENT_WINDOW_DAYS: i64 = 365;
/// Budget usage percentage that budget events report when the query names none.
pub const DEFAULT_BUDGET_THRESHOLD_PERCENT: i64 = 80;
/// Most keys whose own budgets one events query evaluates.
pub const MAX_EVENT_BUDGET_KEYS: i64 = 500;
/// Marker that replaces a redacted value.
pub const REDACTED_MARKER: &str = "[redacted]";
/// Audit target type of team events.
pub const AUDIT_TEAM_TARGET: &str = "team";
/// Audit target type of key events.
pub const AUDIT_KEY_TARGET: &str = "gateway_api_key";
/// Audit event written when an administrator revokes a key.
pub const EVENT_KEY_ADMIN_REVOKED: &str = "gateway_api_key_admin_revoked";
/// Audit event written when an administrator disables a key.
pub const EVENT_KEY_ADMIN_DISABLED: &str = "gateway_api_key_admin_disabled";
/// Audit event written when an administrator enables a key.
pub const EVENT_KEY_ADMIN_ENABLED: &str = "gateway_api_key_admin_enabled";
/// Audit event written when team gateway limits change.
pub const EVENT_TEAM_LIMITS_UPDATED: &str = "gateway_team_limits_updated";
/// Audit event written when team gateway access changes.
pub const EVENT_TEAM_ACCESS_UPDATED: &str = "gateway_team_access_updated";
/// Audit event written when a team's model access changes.
pub const EVENT_TEAM_MODELS_UPDATED: &str = "team_models_updated";
/// Audit event written when a team's budget assignment changes.
pub const EVENT_TEAM_BUDGET_UPDATED: &str = "team_budget_updated";

const SENSITIVE_KEY_PARTS: [&str; 9] = ["secret", "password", "hash", "authorization", "credential", "prompt", "content", "bearer", "cookie"];

// Request Types

#[derive(Debug, Default, Deserialize)]
/// Filters and paging of the all-keys listing.
pub struct GatewayAdminKeyQuery {
	pub team_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub status: Option<String>,
	pub expires_from: Option<DateTime<Utc>>,
	pub expires_to: Option<DateTime<Utc>>,
	pub last_used_before: Option<DateTime<Utc>>,
	pub never_used: Option<bool>,
	pub search: Option<String>,
	pub page: Option<i64>,
	pub size: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
/// Optional reason attached to a key action.
pub struct GatewayAdminActionRequest {
	pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
/// Body replacing a team's gateway rate rules and concurrency cap.
pub struct SetGatewayTeamLimitsRequest {
	pub rules: Vec<GatewayKeyRuleRequest>,
	pub max_concurrent_requests: Option<i32>,
	pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
/// Body turning gateway access on or off for a team.
pub struct SetGatewayTeamAccessRequest {
	pub enabled: bool,
	pub reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
/// Filters and paging of the team listing.
pub struct GatewayAdminTeamQuery {
	pub search: Option<String>,
	pub page: Option<i64>,
	pub size: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
/// Filters and paging of the request metadata listing.
pub struct GatewayAdminRequestQuery {
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub status: Option<String>,
	pub error_category: Option<String>,
	pub model: Option<String>,
	pub provider: Option<String>,
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
	pub page: Option<i64>,
	pub limit: Option<i64>,
}

impl GatewayAdminRequestQuery {
	#[must_use]
	/// The request log filter implied by this query, optionally pinned to one key.
	pub fn filter(&self, api_key_id: Option<Uuid>) -> RequestLogFilter {
		RequestLogFilter {
			user_id: self.user_id,
			team_id: self.team_id,
			api_key_id: api_key_id.or(self.api_key_id),
			status: self.status.as_deref().and_then(RequestLogStatus::from_stored),
			error_category: self.error_category.as_deref().and_then(RequestLogErrorCategory::from_stored),
			model: RequestLogFilter::text(self.model.as_deref()),
			provider: RequestLogFilter::text(self.provider.as_deref()),
			from: self.from,
			to: self.to,
		}
	}
}

#[derive(Debug, Default, Deserialize)]
/// Filters and paging of the audit trail.
pub struct GatewayAuditQuery {
	pub category: Option<String>,
	pub event: Option<String>,
	pub actor_id: Option<Uuid>,
	pub target_id: Option<Uuid>,
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
	pub page: Option<i64>,
	pub size: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
/// Selects which gateway events to compute.
pub struct GatewayEventQuery {
	pub kind: Option<String>,
	pub days: Option<i64>,
	pub threshold_percent: Option<i64>,
}

// Response Types

#[derive(Debug, Serialize, FromRow)]
/// A key as shown to an administrator; never includes the secret or its hash.
pub struct GatewayAdminKeyResponse {
	pub id: Uuid,
	pub name: String,
	pub team_id: Uuid,
	pub team_name: String,
	pub user_id: Uuid,
	pub owner_username: String,
	pub key_prefix: String,
	pub last_four: String,
	pub scopes: Json<Vec<String>>,
	pub status: String,
	pub is_enabled: bool,
	pub admin_disabled: bool,
	pub expires_at: Option<DateTime<Utc>>,
	pub revoked_at: Option<DateTime<Utc>>,
	pub last_used_at: Option<DateTime<Utc>>,
	pub max_concurrent_requests: Option<i32>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// A key with its effective limits, budgets and usage.
pub struct GatewayAdminKeyDetailResponse {
	pub key: GatewayAdminKeyResponse,
	pub team_gateway_enabled: bool,
	pub team_max_concurrent_requests: Option<i32>,
	pub team_rules: Vec<GatewayKeyRuleResponse>,
	pub key_rules: Vec<GatewayKeyRuleResponse>,
	pub budgets: Vec<EffectiveBudgetResponse>,
	pub usage: RequestLogSummary,
}

#[derive(Debug, Serialize)]
/// A team's gateway switches and limits.
pub struct GatewayAdminTeamResponse {
	pub id: Uuid,
	pub name: String,
	pub is_default: bool,
	pub gateway_enabled: bool,
	pub max_concurrent_requests: Option<i32>,
	pub rules: Vec<GatewayKeyRuleResponse>,
	pub key_count: i64,
	pub active_key_count: i64,
}

#[derive(Debug, Serialize)]
/// One audit trail entry with redacted metadata.
pub struct GatewayAuditEntryResponse {
	pub id: Uuid,
	pub event: String,
	pub category: String,
	pub actor_id: Option<Uuid>,
	pub actor_username: Option<String>,
	pub target_type: Option<String>,
	pub target_id: Option<Uuid>,
	pub target_label: Option<String>,
	pub reason: Option<String>,
	pub before: Option<Value>,
	pub after: Option<Value>,
	pub metadata: Value,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// A query-able operational event, ready to forward to a webhook.
pub struct GatewayEventResponse {
	pub kind: String,
	pub subject_type: String,
	pub subject_id: Uuid,
	pub subject_label: String,
	pub team_id: Option<Uuid>,
	pub expires_at: Option<DateTime<Utc>>,
	pub amount: Option<Decimal>,
	pub spent: Option<Decimal>,
	pub percent_used: Option<Decimal>,
}

// Internal Types

/// Failures produced by gateway administration.
#[derive(Debug, thiserror::Error)]
pub enum GatewayAdminError {
	/// The key does not exist.
	#[error("gateway_admin_key_not_found")]
	KeyNotFound,
	/// The key is revoked and can no longer change.
	#[error("gateway_admin_key_revoked")]
	KeyRevoked,
	/// The team does not exist.
	#[error("gateway_admin_team_not_found")]
	TeamNotFound,
	/// The reason is longer than allowed.
	#[error("gateway_admin_reason_invalid")]
	ReasonInvalid,
	/// A rate rule cannot be enforced.
	#[error("gateway_admin_rule_invalid")]
	InvalidRule,
	/// Two rules share a route class, metric and window.
	#[error("gateway_admin_rule_duplicate")]
	DuplicateRule,
	/// The concurrency cap is not positive.
	#[error("gateway_admin_concurrency_invalid")]
	InvalidConcurrency,
	/// A filter value is not supported.
	#[error("gateway_admin_query_invalid")]
	InvalidQuery,
	/// Storage failed.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

impl From<GatewayKeyError> for GatewayAdminError {
	fn from(error: GatewayKeyError) -> Self {
		match error {
			GatewayKeyError::NotFound => Self::KeyNotFound,
			GatewayKeyError::Revoked => Self::KeyRevoked,
			GatewayKeyError::InvalidRule => Self::InvalidRule,
			GatewayKeyError::DuplicateRule => Self::DuplicateRule,
			GatewayKeyError::InvalidConcurrency => Self::InvalidConcurrency,
			GatewayKeyError::Database(error) => Self::Database(error),
			_ => Self::InvalidQuery,
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// What an administrator does to a key.
pub enum GatewayKeyAdminAction {
	Revoke,
	Disable,
	Enable,
}

pub(super) struct AdminAudit<'a> {
	pub event: &'a str,
	pub actor_id: &'a Uuid,
	pub target_type: &'a str,
	pub target_id: &'a Uuid,
	pub reason: Option<&'a str>,
}

/// Validates an optional administrator reason and trims it.
///
/// # Errors
///
/// Returns [`GatewayAdminError::ReasonInvalid`] when the reason is too long.
pub fn normalize_reason(reason: Option<&str>) -> Result<Option<String>, GatewayAdminError> {
	let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) else {
		return Ok(None);
	};
	if reason.chars().count() > MAX_ADMIN_REASON_LENGTH {
		return Err(GatewayAdminError::ReasonInvalid);
	}
	Ok(Some(reason.to_owned()))
}

fn is_sensitive_key(key: &str) -> bool {
	let key = key.to_ascii_lowercase();
	SENSITIVE_KEY_PARTS.iter().any(|part| key.contains(part)) || key == "token" || key.ends_with("_token") || key == "api_key"
}

fn is_sensitive_text(text: &str) -> bool {
	(text.starts_with("oxc_") && text.matches('_').count() >= 2) || text.to_ascii_lowercase().starts_with("bearer ")
}

#[must_use]
/// Replaces secrets, credentials and request content in audit metadata with a marker.
pub fn redact_metadata(value: &Value) -> Value {
	match value {
		Value::Object(map) => Value::Object(
			map.iter()
				.map(|(key, value)| {
					let redacted = if is_sensitive_key(key) { Value::String(REDACTED_MARKER.to_owned()) } else { redact_metadata(value) };
					(key.clone(), redacted)
				})
				.collect::<Map<String, Value>>(),
		),
		Value::Array(items) => Value::Array(items.iter().map(redact_metadata).collect()),
		Value::String(text) if is_sensitive_text(text) => Value::String(REDACTED_MARKER.to_owned()),
		other => other.clone(),
	}
}

pub(super) async fn write_admin_audit(tx: &mut Transaction<'_, Postgres>, entry: &AdminAudit<'_>, metadata: Value) -> Result<(), sqlx::Error> {
	let mut metadata = match redact_metadata(&metadata) {
		Value::Object(map) => map,
		other => Map::from_iter([("details".to_owned(), other)]),
	};
	if let Some(reason) = entry.reason {
		metadata.insert("reason".to_owned(), redact_metadata(&Value::String(reason.to_owned())));
	}
	metadata.insert("admin_action".to_owned(), Value::Bool(true));
	sqlx::query("INSERT INTO audit_logs (event, actor_id, target_type, target_id, metadata) VALUES ($1, $2, $3, $4, $5)")
		.bind(entry.event)
		.bind(entry.actor_id)
		.bind(entry.target_type)
		.bind(entry.target_id)
		.bind(Value::Object(metadata))
		.execute(&mut **tx)
		.await?;
	Ok(())
}
