//! Standardized response utilities for Axum handlers.
//!
//! Provides a fluent builder pattern for HTTP responses with support for
//! custom headers, various content types, caching, CORS, and structured errors.

use axum::{
	Json,
	body::Body,
	http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;

use crate::i18n::I18n;

// ============================================================================
// Response Builder
// ============================================================================

/// Fluent builder for HTTP responses with custom headers and content types.
pub struct ResponseBuilder<T: Serialize> {
	status: StatusCode,
	headers: HeaderMap,
	body: ResponseBody<T>,
}

pub enum ResponseBody<T: Serialize> {
	Json(T),
	Text(String),
	Empty,
}

impl<T: Serialize> ResponseBuilder<T> {
	pub fn new(body: ResponseBody<T>) -> Self {
		Self {
			status: StatusCode::OK,
			headers: HeaderMap::new(),
			body,
		}
	}

	/// Set the HTTP status code.
	#[must_use]
	pub fn status(mut self, status: StatusCode) -> Self {
		self.status = status;
		self
	}

	/// Add a single header.
	#[must_use]
	pub fn header(mut self, key: HeaderName, value: HeaderValue) -> Self {
		self.headers.insert(key, value);
		self
	}

	/// Add a header from string values.
	///
	/// # Panics
	///
	/// Panics if the key or value cannot be parsed as valid header components.
	#[must_use]
	pub fn header_str(mut self, key: &'static str, value: &str) -> Self {
		self.headers
			.insert(HeaderName::from_static(key), HeaderValue::from_str(value).expect("Invalid header value"));
		self
	}

	/// Add multiple headers from a `HeaderMap`.
	#[must_use]
	pub fn headers(mut self, headers: HeaderMap) -> Self {
		self.headers.extend(headers);
		self
	}

	/// Set the `Content-Type` header.
	#[must_use]
	pub fn content_type(self, content_type: &str) -> Self {
		self.header(header::CONTENT_TYPE, HeaderValue::from_str(content_type).expect("Invalid content type"))
	}

	/// Set a `Cache-Control` directive.
	#[must_use]
	pub fn cache_control(self, directive: &str) -> Self {
		self.header(header::CACHE_CONTROL, HeaderValue::from_str(directive).expect("Invalid cache directive"))
	}

	/// Set `Cache-Control: no-store, no-cache, must-revalidate`.
	#[must_use]
	pub fn no_cache(self) -> Self {
		self.cache_control("no-store, no-cache, must-revalidate")
	}

	/// Set `Cache-Control: max-age=<seconds>`.
	#[must_use]
	pub fn cache_max_age(self, seconds: u32) -> Self {
		self.cache_control(&format!("max-age={seconds}"))
	}

	/// Set `Cache-Control: private, max-age=<seconds>`.
	#[must_use]
	pub fn cache_private(self, seconds: u32) -> Self {
		self.cache_control(&format!("private, max-age={seconds}"))
	}

	/// Set `Access-Control-Allow-Origin` header.
	#[must_use]
	pub fn allow_origin(self, origin: &str) -> Self {
		self.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(origin).expect("Invalid origin"))
	}

	/// Set permissive CORS headers (allow all origins, methods, headers).
	#[must_use]
	pub fn cors_permissive(mut self) -> Self {
		self.headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
		self.headers.insert(
			header::ACCESS_CONTROL_ALLOW_METHODS,
			HeaderValue::from_static("GET, POST, PUT, DELETE, PATCH, OPTIONS"),
		);
		self.headers
			.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("Content-Type, Authorization"));
		self
	}

	/// Add common security headers.
	#[must_use]
	pub fn security_headers(mut self) -> Self {
		self.headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
		self.headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
		self.headers.insert(header::X_XSS_PROTECTION, HeaderValue::from_static("1; mode=block"));
		self.headers.insert(
			HeaderName::from_static("referrer-policy"),
			HeaderValue::from_static("strict-origin-when-cross-origin"),
		);
		self
	}

	/// Build the final response.
	#[must_use]
	pub fn build(self) -> Response {
		let mut response = match self.body {
			ResponseBody::Json(data) => Json(data).into_response(),
			ResponseBody::Text(content) => {
				let mut resp = Response::new(Body::from(content));
				resp.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
				resp
			}
			ResponseBody::Empty => Response::new(Body::empty()),
		};

		*response.status_mut() = self.status;
		response.headers_mut().extend(self.headers);
		response
	}
}

impl<T: Serialize> IntoResponse for ResponseBuilder<T> {
	fn into_response(self) -> Response {
		self.build()
	}
}

// ============================================================================
// Error Codes
// ============================================================================

/// Typed error codes with HTTP status mapping and i18n keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
	// 400 Bad Request
	BadRequest,
	ValidationFailed,
	InvalidEmail,
	UsernameInvalid,
	UsernameTooShort,
	UsernameTooLong,
	PasswordTooShort,
	PasswordTooLong,
	PasswordNoUppercase,
	PasswordNoLowercase,
	PasswordNoDigit,
	PasswordNoSpecial,
	SetupRequired,
	SetupCompleted,
	MalformedRequest,
	InvalidProvider,
	ProviderNotConfigured,

	// 401 Unauthorized
	Unauthorized,
	NotAuthenticated,
	InvalidCredentials,
	SessionExpired,
	TokenInvalid,
	TokenExpired,
	ExternalAuthRequired,
	OAuthStateMismatch,
	OAuthTokenError,
	OAuthUserInfoError,

	// 403 Forbidden
	Forbidden,
	InsufficientPermissions,
	AccountDisabled,

	// 404 Not Found
	NotFound,
	UserNotFound,
	ResourceNotFound,
	TranslationNotFound,

	// 409 Conflict
	Conflict,
	EmailTaken,
	UsernameTaken,
	EmailOrUsernameTaken,
	AlreadyExists,

	// 422 Unprocessable Entity
	UnprocessableEntity,

	// 429 Too Many Requests
	RateLimited,

	// 500+ Server Errors
	InternalError,
	DatabaseError,
	ServiceUnavailable,
}

impl ErrorCode {
	/// Get the HTTP status code for this error.
	#[must_use]
	pub const fn status(&self) -> StatusCode {
		match self {
			// 400
			Self::BadRequest
			| Self::ValidationFailed
			| Self::InvalidEmail
			| Self::UsernameInvalid
			| Self::UsernameTooShort
			| Self::UsernameTooLong
			| Self::PasswordTooShort
			| Self::PasswordTooLong
			| Self::PasswordNoUppercase
			| Self::PasswordNoLowercase
			| Self::PasswordNoDigit
			| Self::PasswordNoSpecial
			| Self::SetupRequired
			| Self::SetupCompleted
			| Self::MalformedRequest
			| Self::InvalidProvider
			| Self::ProviderNotConfigured => StatusCode::BAD_REQUEST,

			Self::Unauthorized
			| Self::NotAuthenticated
			| Self::InvalidCredentials
			| Self::SessionExpired
			| Self::TokenInvalid
			| Self::TokenExpired
			| Self::ExternalAuthRequired
			| Self::OAuthStateMismatch
			| Self::OAuthTokenError
			| Self::OAuthUserInfoError => StatusCode::UNAUTHORIZED,

			// 403
			Self::Forbidden | Self::InsufficientPermissions | Self::AccountDisabled => StatusCode::FORBIDDEN,

			// 404
			Self::NotFound | Self::UserNotFound | Self::ResourceNotFound | Self::TranslationNotFound => StatusCode::NOT_FOUND,

			// 409
			Self::Conflict | Self::EmailTaken | Self::UsernameTaken | Self::EmailOrUsernameTaken | Self::AlreadyExists => StatusCode::CONFLICT,

			// 422
			Self::UnprocessableEntity => StatusCode::UNPROCESSABLE_ENTITY,

			// 429
			Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,

			// 500+
			Self::InternalError | Self::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
			Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
		}
	}

	/// Get the i18n key for this error's message.
	#[must_use]
	pub const fn i18n_key(&self) -> &'static str {
		match self {
			Self::BadRequest => "errors.bad_request",
			Self::ValidationFailed => "errors.validation_failed",
			Self::InvalidEmail => "auth.errors.invalid_email",
			Self::UsernameInvalid => "auth.errors.username_invalid_chars",
			Self::UsernameTooShort => "auth.errors.username_too_short",
			Self::UsernameTooLong => "auth.errors.username_too_long",
			Self::PasswordTooShort => "auth.errors.password_too_short",
			Self::PasswordTooLong => "auth.errors.password_too_long",
			Self::PasswordNoUppercase => "auth.errors.password_no_uppercase",
			Self::PasswordNoLowercase => "auth.errors.password_no_lowercase",
			Self::PasswordNoDigit => "auth.errors.password_no_digit",
			Self::PasswordNoSpecial => "auth.errors.password_no_special",
			Self::SetupRequired => "auth.errors.setup_required",
			Self::SetupCompleted => "auth.errors.setup_completed",
			Self::MalformedRequest => "errors.malformed_request",
			Self::InvalidProvider => "auth.errors.oauth_provider_invalid",
			Self::ProviderNotConfigured => "auth.errors.oauth_provider_disabled",

			Self::Unauthorized => "auth.errors.unauthorized",
			Self::NotAuthenticated => "auth.errors.not_authenticated",
			Self::InvalidCredentials => "auth.errors.invalid_credentials",
			Self::SessionExpired => "auth.errors.session_expired",
			Self::TokenInvalid => "auth.errors.token_invalid",
			Self::TokenExpired => "auth.errors.token_expired",
			Self::ExternalAuthRequired => "auth.errors.external_auth",
			Self::OAuthStateMismatch => "auth.errors.oauth_state_mismatch",
			Self::OAuthTokenError => "auth.errors.oauth_token_error",
			Self::OAuthUserInfoError => "auth.errors.oauth_user_info_error",

			Self::Forbidden => "errors.forbidden",
			Self::InsufficientPermissions => "errors.insufficient_permissions",
			Self::AccountDisabled => "auth.errors.account_disabled",

			Self::NotFound => "errors.not_found",
			Self::UserNotFound => "auth.errors.user_not_found",
			Self::ResourceNotFound => "errors.resource_not_found",
			Self::TranslationNotFound => "i18n.errors.translation_not_found",

			Self::Conflict => "errors.conflict",
			Self::EmailTaken => "auth.errors.email_taken",
			Self::UsernameTaken => "auth.errors.username_taken",
			Self::EmailOrUsernameTaken => "auth.errors.email_or_username_taken",
			Self::AlreadyExists => "errors.already_exists",

			Self::UnprocessableEntity => "errors.unprocessable_entity",

			Self::RateLimited => "errors.rate_limited",

			Self::InternalError | Self::DatabaseError => "auth.errors.internal_error",
			Self::ServiceUnavailable => "errors.service_unavailable",
		}
	}

	/// Get the error code string (snake_case).
	#[must_use]
	pub const fn code(&self) -> &'static str {
		match self {
			Self::BadRequest => "bad_request",
			Self::ValidationFailed => "validation_failed",
			Self::InvalidEmail => "invalid_email",
			Self::UsernameInvalid => "username_invalid",
			Self::UsernameTooShort => "username_too_short",
			Self::UsernameTooLong => "username_too_long",
			Self::PasswordTooShort => "password_too_short",
			Self::PasswordTooLong => "password_too_long",
			Self::PasswordNoUppercase => "password_no_uppercase",
			Self::PasswordNoLowercase => "password_no_lowercase",
			Self::PasswordNoDigit => "password_no_digit",
			Self::PasswordNoSpecial => "password_no_special",
			Self::SetupRequired => "setup_required",
			Self::SetupCompleted => "setup_completed",
			Self::MalformedRequest => "malformed_request",
			Self::InvalidProvider => "invalid_provider",
			Self::ProviderNotConfigured => "provider_not_configured",

			Self::Unauthorized => "unauthorized",
			Self::NotAuthenticated => "not_authenticated",
			Self::InvalidCredentials => "invalid_credentials",
			Self::SessionExpired => "session_expired",
			Self::TokenInvalid => "token_invalid",
			Self::TokenExpired => "token_expired",
			Self::ExternalAuthRequired => "external_auth_required",
			Self::OAuthStateMismatch => "oauth_state_mismatch",
			Self::OAuthTokenError => "oauth_token_error",
			Self::OAuthUserInfoError => "oauth_user_info_error",

			Self::Forbidden => "forbidden",
			Self::InsufficientPermissions => "insufficient_permissions",
			Self::AccountDisabled => "account_disabled",

			Self::NotFound => "not_found",
			Self::UserNotFound => "user_not_found",
			Self::ResourceNotFound => "resource_not_found",
			Self::TranslationNotFound => "translation_not_found",

			Self::Conflict => "conflict",
			Self::EmailTaken => "email_taken",
			Self::UsernameTaken => "username_taken",
			Self::EmailOrUsernameTaken => "email_or_username_taken",
			Self::AlreadyExists => "already_exists",

			Self::UnprocessableEntity => "unprocessable_entity",

			Self::RateLimited => "rate_limited",

			Self::InternalError => "internal_error",
			Self::DatabaseError => "database_error",
			Self::ServiceUnavailable => "service_unavailable",
		}
	}

	/// Get the translated message for this error.
	#[must_use]
	pub fn message(&self) -> String {
		I18n::get().translate(self.i18n_key(), &None)
	}
}

// ============================================================================
// Error Types
// ============================================================================

/// A single error detail in an API error response.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
	pub code: &'static str,
	pub message: String,
	pub status: u16,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub details: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub field: Option<String>,
}

/// Structured API error response containing one or more errors.
#[derive(Debug, Serialize)]
pub struct ApiError {
	pub errors: Vec<ErrorDetail>,
}

impl ApiError {
	/// Create an error response with a single error.
	#[must_use]
	pub fn single(code: ErrorCode) -> Self {
		Self {
			errors: vec![ErrorDetail {
				code: code.code(),
				message: code.message(),
				status: code.status().as_u16(),
				details: None,
				field: None,
			}],
		}
	}

	/// Create an error response with details.
	#[must_use]
	pub fn with_details(code: ErrorCode, details: Value) -> Self {
		Self {
			errors: vec![ErrorDetail {
				code: code.code(),
				message: code.message(),
				status: code.status().as_u16(),
				details: Some(details),
				field: None,
			}],
		}
	}

	/// Create an error response with a field annotation.
	#[must_use]
	pub fn with_field(code: ErrorCode, field: impl Into<String>) -> Self {
		Self {
			errors: vec![ErrorDetail {
				code: code.code(),
				message: code.message(),
				status: code.status().as_u16(),
				details: None,
				field: Some(field.into()),
			}],
		}
	}

	/// Create an error response with multiple errors.
	#[must_use]
	pub fn multiple(codes: Vec<ErrorCode>) -> Self {
		Self {
			errors: codes
				.into_iter()
				.map(|code| ErrorDetail {
					code: code.code(),
					message: code.message(),
					status: code.status().as_u16(),
					details: None,
					field: None,
				})
				.collect(),
		}
	}

	/// Get the primary status code (from first error, or 500).
	pub fn primary_status(&self) -> StatusCode {
		self.errors.first().map_or(StatusCode::INTERNAL_SERVER_ERROR, |e| {
			StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
		})
	}
}

// ============================================================================
// Error Builder
// ============================================================================

/// Fluent builder for constructing complex error responses.
pub struct ErrorBuilder {
	errors: Vec<ErrorDetail>,
	headers: HeaderMap,
	current_details: Option<Value>,
	current_field: Option<String>,
}

impl ErrorBuilder {
	/// Create a new error builder with an initial error code.
	#[must_use]
	pub fn new(code: ErrorCode) -> Self {
		Self {
			errors: vec![ErrorDetail {
				code: code.code(),
				message: code.message(),
				status: code.status().as_u16(),
				details: None,
				field: None,
			}],
			headers: HeaderMap::new(),
			current_details: None,
			current_field: None,
		}
	}

	/// Add another error to the response.
	#[must_use]
	pub fn add(mut self, code: ErrorCode) -> Self {
		// Apply pending details/field to previous error
		self.finalize_current();

		self.errors.push(ErrorDetail {
			code: code.code(),
			message: code.message(),
			status: code.status().as_u16(),
			details: None,
			field: None,
		});
		self
	}

	/// Add details to the current (last) error.
	#[must_use]
	pub fn details(mut self, details: Value) -> Self {
		self.current_details = Some(details);
		self
	}

	/// Add a field annotation to the current (last) error.
	#[must_use]
	pub fn field(mut self, field: impl Into<String>) -> Self {
		self.current_field = Some(field.into());
		self
	}

	/// Add a response header.
	#[must_use]
	pub fn header(mut self, key: HeaderName, value: HeaderValue) -> Self {
		self.headers.insert(key, value);
		self
	}

	/// Add a response header from string values.
	///
	/// # Panics
	///
	/// Panics if the key or value cannot be parsed as valid header components.
	#[must_use]
	pub fn header_str(mut self, key: &'static str, value: &str) -> Self {
		self.headers
			.insert(HeaderName::from_static(key), HeaderValue::from_str(value).expect("Invalid header value"));
		self
	}

	fn finalize_current(&mut self) {
		if let Some(last) = self.errors.last_mut() {
			if let Some(details) = self.current_details.take() {
				last.details = Some(details);
			}
			if let Some(field) = self.current_field.take() {
				last.field = Some(field);
			}
		}
	}

	/// Build the final error response.
	#[must_use]
	pub fn build(mut self) -> Response {
		self.finalize_current();

		let status = self.errors.first().map_or(StatusCode::INTERNAL_SERVER_ERROR, |e| {
			StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
		});

		let api_error = ApiError { errors: self.errors };
		let mut response = (status, Json(api_error)).into_response();
		response.headers_mut().extend(self.headers);
		response
	}
}

impl IntoResponse for ErrorBuilder {
	fn into_response(self) -> Response {
		self.build()
	}
}
