use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod repository;

/// Days a request log row is kept when the user has no retention setting.
pub const DEFAULT_REQUEST_LOG_RETENTION_DAYS: i32 = 30;
/// Largest retention a user may configure.
pub const MAX_REQUEST_LOG_RETENTION_DAYS: i32 = 3650;
/// Most rows a single export returns.
pub const REQUEST_LOG_EXPORT_LIMIT: i64 = 10_000;

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

#[derive(Debug, Clone, sqlx::FromRow)]
/// One logged gateway request, without prompt or response content.
pub struct RequestLog {
	pub id: Uuid,
	pub user_id: Uuid,
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
}

pub struct RequestLogRecord<'a> {
	pub user_id: Uuid,
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
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RequestLogFilter {
	pub user_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub status: Option<RequestLogStatus>,
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
}

// Request Types

#[derive(Debug, Deserialize)]
pub struct RequestLogQuery {
	pub team_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub status: Option<String>,
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
			from: self.from,
			to: self.to,
		}
	}
}

// Response Types

#[derive(Debug, Serialize)]
/// A request log as shown to its owner; never includes request content.
pub struct RequestLogResponse {
	pub id: Uuid,
	pub user_id: Uuid,
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
		}
	}
}
