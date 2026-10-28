use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

mod repository;

/// Days a request log row is kept when the user has no retention setting.
pub const DEFAULT_REQUEST_LOG_RETENTION_DAYS: i32 = 30;
/// Largest retention a user may configure.
pub const MAX_REQUEST_LOG_RETENTION_DAYS: i32 = 3650;
/// Most rows a single export returns.
pub const REQUEST_LOG_EXPORT_LIMIT: i64 = 10_000;
/// Request metadata key carrying the request id to usage reporting.
pub const REQUEST_ID_METADATA_KEY: &str = "oxide_request_id";
/// Request metadata key carrying whether the request streams its response.
pub const STREAMING_METADATA_KEY: &str = "oxide_streaming";
/// Request metadata key carrying the request start time in unix milliseconds.
pub const STARTED_AT_METADATA_KEY: &str = "oxide_started_at_ms";

#[derive(Clone, Copy, Debug)]
/// Identity and start time of one gateway request, shared through request extensions.
pub struct RequestLogContext {
	pub request_id: Uuid,
	pub started_at: DateTime<Utc>,
}

impl RequestLogContext {
	#[must_use]
	/// Starts a context with a fresh request id at the current time.
	pub fn start() -> Self {
		Self {
			request_id: Uuid::new_v4(),
			started_at: Utc::now(),
		}
	}

	#[must_use]
	/// Whole milliseconds between the start and the given time, never negative.
	pub fn latency_ms(&self, finished_at: DateTime<Utc>) -> i32 {
		Self::latency_between(self.started_at, finished_at)
	}

	#[must_use]
	/// Whole milliseconds from `started_at` to `finished_at`, never negative.
	pub fn latency_between(started_at: DateTime<Utc>, finished_at: DateTime<Utc>) -> i32 {
		let millis = (finished_at - started_at).num_milliseconds().max(0);
		i32::try_from(millis).unwrap_or(i32::MAX)
	}

	#[must_use]
	/// Parses a start time stored in request metadata as unix milliseconds.
	pub fn parse_started_at(value: &str) -> Option<DateTime<Utc>> {
		value.parse::<i64>().ok().and_then(DateTime::from_timestamp_millis)
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Outcome of one logged request.
pub enum RequestLogStatus {
	Success,
	Error,
}

impl RequestLogStatus {
	#[must_use]
	/// The stored value for this status.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Success => "success",
			Self::Error => "error",
		}
	}

	#[must_use]
	/// Parses a stored or requested status; unknown values yield `None`.
	pub fn from_stored(value: &str) -> Option<Self> {
		match value {
			"success" => Some(Self::Success),
			"error" => Some(Self::Error),
			_ => None,
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Why a logged request failed.
pub enum RequestLogErrorCategory {
	Upstream,
	Interrupted,
	Budget,
	RateLimit,
	Auth,
	ModelAccess,
	Internal,
}

impl RequestLogErrorCategory {
	#[must_use]
	/// The stored value for this category.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Upstream => "upstream",
			Self::Interrupted => "interrupted",
			Self::Budget => "budget",
			Self::RateLimit => "rate_limit",
			Self::Auth => "auth",
			Self::ModelAccess => "model_access",
			Self::Internal => "internal",
		}
	}

	#[must_use]
	/// Parses a stored or requested category; unknown values yield `None`.
	pub fn from_stored(value: &str) -> Option<Self> {
		match value {
			"upstream" => Some(Self::Upstream),
			"interrupted" => Some(Self::Interrupted),
			"budget" => Some(Self::Budget),
			"rate_limit" => Some(Self::RateLimit),
			"auth" => Some(Self::Auth),
			"model_access" => Some(Self::ModelAccess),
			"internal" => Some(Self::Internal),
			_ => None,
		}
	}
}

#[derive(Clone, Copy, Debug)]
/// The account a logged request belongs to.
pub struct RequestLogOwner {
	pub user_id: Option<Uuid>,
	pub team_id: Uuid,
	pub api_key_id: Uuid,
}

#[derive(Debug)]
/// Everything known about a gateway request before it produces a log row.
pub struct RequestLogTrace<'a> {
	pub context: RequestLogContext,
	pub owner: RequestLogOwner,
	pub request_type: &'a str,
	pub public_model_id: &'a str,
	pub model_id: Option<Uuid>,
	pub provider: Option<&'a str>,
	pub streaming: bool,
}

#[derive(Debug)]
/// Owned description of a response, used to log it when the client drops it before it ends.
pub struct RequestLogAbort {
	pool: PgPool,
	context: RequestLogContext,
	owner: RequestLogOwner,
	request_type: String,
	public_model_id: String,
	model_id: Option<Uuid>,
	provider: Option<String>,
	streaming: bool,
}

#[derive(Clone, Copy, Debug)]
/// A failed request's category and optional HTTP status.
pub struct RequestLogFailure {
	pub category: RequestLogErrorCategory,
	pub status_code: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
/// One logged gateway request, without prompt or response content.
pub struct RequestLog {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub public_model_id: String,
	pub request_type: String,
	pub status: String,
	pub status_code: Option<i32>,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
	pub created_at: DateTime<Utc>,
	pub request_id: Option<Uuid>,
	pub provider: Option<String>,
	pub error_category: Option<String>,
	pub streaming: bool,
	pub latency_ms: Option<i32>,
	pub finish_reason: Option<String>,
	pub time_to_first_token_ms: Option<i32>,
}

pub struct RequestLogRecord<'a> {
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub public_model_id: &'a str,
	pub request_type: &'a str,
	pub status: RequestLogStatus,
	pub status_code: Option<i32>,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
	pub request_id: Option<Uuid>,
	pub provider: Option<&'a str>,
	pub error_category: Option<RequestLogErrorCategory>,
	pub streaming: bool,
	pub latency_ms: Option<i32>,
	pub finish_reason: Option<&'a str>,
	pub time_to_first_token_ms: Option<i32>,
}

pub const REQUEST_LOG_SCOPE_TEAM_SERVICE: &str = "team_service";

#[derive(Debug, thiserror::Error)]
pub enum RequestLogScopeError {
	#[error("request log scope is forbidden")]
	Forbidden,
	#[error("request log scope requires team admin")]
	AdminRequired,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Default)]
pub struct RequestLogFilter {
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub status: Option<RequestLogStatus>,
	pub error_category: Option<RequestLogErrorCategory>,
	pub model: Option<String>,
	pub provider: Option<String>,
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
	pub service_accounts_only: bool,
}

impl RequestLogFilter {
	#[must_use]
	/// Trims a requested text filter and drops it when nothing is left.
	pub fn text(value: Option<&str>) -> Option<String> {
		value.map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
	}
}

// Request Types

#[derive(Debug, Deserialize)]
pub struct RequestLogQuery {
	pub scope: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct SetRequestLogRetentionRequest {
	pub retention_days: i32,
}

impl RequestLogQuery {
	#[must_use]
	/// The filter implied by this query, narrowed to the supplied user.
	pub fn filter(&self, user_id: Uuid) -> RequestLogFilter {
		RequestLogFilter {
			user_id: Some(user_id),
			team_id: self.team_id,
			api_key_id: self.api_key_id,
			status: self.status.as_deref().and_then(RequestLogStatus::from_stored),
			error_category: self.error_category.as_deref().and_then(RequestLogErrorCategory::from_stored),
			model: RequestLogFilter::text(self.model.as_deref()),
			provider: RequestLogFilter::text(self.provider.as_deref()),
			from: self.from,
			to: self.to,
			service_accounts_only: false,
		}
	}
}

// Response Types

#[derive(Debug, Serialize)]
/// A request log as shown to its owner; never includes request content.
pub struct RequestLogResponse {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub public_model_id: String,
	pub request_type: String,
	pub status: String,
	pub status_code: Option<i32>,
	pub input_tokens: i32,
	pub output_tokens: i32,
	pub reasoning_tokens: i32,
	pub cost_total: Decimal,
	pub created_at: DateTime<Utc>,
	pub request_id: Option<Uuid>,
	pub provider: Option<String>,
	pub error_category: Option<String>,
	pub streaming: bool,
	pub latency_ms: Option<i32>,
	pub finish_reason: Option<String>,
	pub time_to_first_token_ms: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
/// Request counts, tokens and spend over the logged requests matching a filter.
pub struct RequestLogSummary {
	pub total_requests: i64,
	pub error_requests: i64,
	pub requests_24h: i64,
	pub input_tokens: i64,
	pub output_tokens: i64,
	pub cost_total: Decimal,
	pub last_request_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
/// The caller's effective request log retention.
pub struct RequestLogRetentionResponse {
	pub retention_days: i32,
}

impl RequestLog {
	#[must_use]
	/// Builds the owner-facing representation of this log row.
	pub fn to_response(&self) -> RequestLogResponse {
		RequestLogResponse {
			id: self.id,
			user_id: self.user_id,
			team_id: self.team_id,
			api_key_id: self.api_key_id,
			model_id: self.model_id,
			public_model_id: self.public_model_id.clone(),
			request_type: self.request_type.clone(),
			status: self.status.clone(),
			status_code: self.status_code,
			input_tokens: self.input_tokens,
			output_tokens: self.output_tokens,
			reasoning_tokens: self.reasoning_tokens,
			cost_total: self.cost_total,
			created_at: self.created_at,
			request_id: self.request_id,
			provider: self.provider.clone(),
			error_category: self.error_category.clone(),
			streaming: self.streaming,
			latency_ms: self.latency_ms,
			finish_reason: self.finish_reason.clone(),
			time_to_first_token_ms: self.time_to_first_token_ms,
		}
	}
}
