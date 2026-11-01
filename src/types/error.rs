//! Handler error type shared by all routes.

use crate::types::{GatewayAdminError, GatewayKeyError, RequestLogScopeError};
use crate::utils::response::{ErrorBuilder, ErrorCode};
use crate::utils::response::{ResponseBody, ResponseBuilder};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

// Internal Types

/// Error returned by handlers; renders the standard JSON error body.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
	/// A client-facing error code.
	#[error("{0:?}")]
	Code(ErrorCode),
	/// A database failure, rendered as an internal error.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
	/// A database failure rendered as a database error.
	#[error(transparent)]
	DatabaseCode(sqlx::Error),
	/// A gateway key management failure.
	#[error(transparent)]
	GatewayKey(#[from] GatewayKeyError),
	/// A gateway administration failure.
	#[error(transparent)]
	GatewayAdmin(#[from] GatewayAdminError),
}

/// Returns true when the database rejected a statement because of a unique constraint.
#[must_use]
pub fn is_unique_violation(error: &sqlx::Error) -> bool {
	error.as_database_error().is_some_and(sqlx::error::DatabaseError::is_unique_violation)
}

/// Wraps a body in a 200 JSON response.
///
/// # Errors
///
/// Never fails; the `Result` lets handlers return it directly.
pub fn json_ok<T: Serialize>(body: T) -> Result<Response, AppError> {
	Ok(ResponseBuilder::new(ResponseBody::Json(body)).build())
}

impl AppError {
	/// Maps a unique violation to `code`, any other database failure to an internal error.
	#[must_use]
	pub fn unique_or(error: sqlx::Error, code: ErrorCode) -> Self {
		if is_unique_violation(&error) { Self::Code(code) } else { Self::Database(error) }
	}

	/// Wraps a database failure so it renders `DatabaseError` and is logged.
	#[must_use]
	pub fn database(error: sqlx::Error) -> Self {
		Self::DatabaseCode(error)
	}

	/// The error code rendered to the client.
	#[must_use]
	pub fn code(&self) -> ErrorCode {
		match self {
			Self::Code(code) => *code,
			Self::Database(_) => ErrorCode::InternalError,
			Self::DatabaseCode(_) => ErrorCode::DatabaseError,
			Self::GatewayKey(error) => key_error_code(error),
			Self::GatewayAdmin(error) => admin_error_code(error),
		}
	}
}

impl From<ErrorCode> for AppError {
	fn from(code: ErrorCode) -> Self {
		Self::Code(code)
	}
}

impl From<RequestLogScopeError> for AppError {
	fn from(error: RequestLogScopeError) -> Self {
		match error {
			RequestLogScopeError::Forbidden => Self::Code(ErrorCode::ApiKeyTeamForbidden),
			RequestLogScopeError::AdminRequired => Self::Code(ErrorCode::ApiKeyTeamAdminRequired),
			RequestLogScopeError::Database(error) => Self::GatewayAdmin(GatewayAdminError::Database(error)),
		}
	}
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		let code = self.code();
		if code.status().is_server_error() {
			tracing::error!(error = %self, "request failed");
		}
		ErrorBuilder::new(code).build()
	}
}

fn key_error_code(error: &GatewayKeyError) -> ErrorCode {
	match error {
		GatewayKeyError::NotFound => ErrorCode::ApiKeyNotFound,
		GatewayKeyError::Revoked => ErrorCode::ApiKeyRevoked,
		GatewayKeyError::AdminDisabled => ErrorCode::ApiKeyAdminDisabled,
		GatewayKeyError::TeamForbidden => ErrorCode::ApiKeyTeamForbidden,
		GatewayKeyError::ServiceAccountForbidden => ErrorCode::ApiKeyServiceAccountForbidden,
		GatewayKeyError::InvalidName => ErrorCode::ApiKeyNameInvalid,
		GatewayKeyError::InvalidScopes => ErrorCode::ApiKeyScopesInvalid,
		GatewayKeyError::InvalidExpiry => ErrorCode::ApiKeyExpiryInvalid,
		GatewayKeyError::InvalidConcurrency => ErrorCode::ApiKeyConcurrencyInvalid,
		GatewayKeyError::InvalidOverlap => ErrorCode::ApiKeyOverlapInvalid,
		GatewayKeyError::InvalidRule => ErrorCode::ApiKeyRuleInvalid,
		GatewayKeyError::InvalidBudget => ErrorCode::ApiKeyBudgetInvalid,
		GatewayKeyError::DuplicateRule => ErrorCode::ApiKeyRuleDuplicate,
		GatewayKeyError::Secret | GatewayKeyError::Database(_) => ErrorCode::InternalError,
	}
}

fn admin_error_code(error: &GatewayAdminError) -> ErrorCode {
	match error {
		GatewayAdminError::KeyNotFound => ErrorCode::ApiKeyNotFound,
		GatewayAdminError::KeyRevoked => ErrorCode::ApiKeyRevoked,
		GatewayAdminError::TeamNotFound => ErrorCode::NotFound,
		GatewayAdminError::ReasonInvalid => ErrorCode::GatewayAdminReasonInvalid,
		GatewayAdminError::InvalidRule => ErrorCode::ApiKeyRuleInvalid,
		GatewayAdminError::DuplicateRule => ErrorCode::ApiKeyRuleDuplicate,
		GatewayAdminError::InvalidConcurrency => ErrorCode::ApiKeyConcurrencyInvalid,
		GatewayAdminError::InvalidQuery => ErrorCode::ValidationFailed,
		GatewayAdminError::Database(_) => ErrorCode::DatabaseError,
	}
}
