#[cfg(test)]
mod tests {
	use axum::http::StatusCode;

	use crate::utils::response::{ErrorCode, ErrorDetail};

	#[test]
	fn error_code_status_mapping() {
		assert_eq!(ErrorCode::BadRequest.status(), StatusCode::BAD_REQUEST);
		assert_eq!(ErrorCode::NotAuthenticated.status(), StatusCode::UNAUTHORIZED);
		assert_eq!(ErrorCode::Forbidden.status(), StatusCode::FORBIDDEN);
		assert_eq!(ErrorCode::NotFound.status(), StatusCode::NOT_FOUND);
		assert_eq!(ErrorCode::EmailTaken.status(), StatusCode::CONFLICT);
		assert_eq!(ErrorCode::RateLimited.status(), StatusCode::TOO_MANY_REQUESTS);
		assert_eq!(ErrorCode::InternalError.status(), StatusCode::INTERNAL_SERVER_ERROR);
	}

	#[test]
	fn error_code_has_code_string() {
		assert_eq!(ErrorCode::InvalidEmail.code(), "invalid_email");
		assert_eq!(ErrorCode::InternalError.code(), "internal_error");
	}

	#[test]
	fn error_code_has_i18n_key() {
		assert_eq!(ErrorCode::InvalidEmail.i18n_key(), "auth.errors.invalid_email");
		assert_eq!(ErrorCode::NotFound.i18n_key(), "errors.not_found");
	}

	#[test]
	fn error_detail_serializes() {
		let detail = ErrorDetail {
			code: ErrorCode::NotFound.code(),
			message: "Not found".to_string(),
			status: ErrorCode::NotFound.status().as_u16(),
			details: None,
			field: None,
		};
		let json = serde_json::to_string(&detail).unwrap();
		assert!(json.contains("\"code\":\"not_found\""));
		assert!(json.contains("\"status\":404"));
	}

	#[test]
	fn error_detail_with_field_serializes() {
		let detail = ErrorDetail {
			code: ErrorCode::InvalidEmail.code(),
			message: "Invalid email".to_string(),
			status: ErrorCode::InvalidEmail.status().as_u16(),
			details: None,
			field: Some("email".to_string()),
		};
		let json = serde_json::to_string(&detail).unwrap();
		assert!(json.contains("\"field\":\"email\""));
	}
}

#[cfg(test)]
mod app_error {
	use crate::types::{AppError, GatewayAdminError, GatewayKeyError};
	use crate::utils::response::ErrorCode;
	use axum::{http::StatusCode, response::IntoResponse};

	#[test]
	fn code_variant_keeps_its_status() {
		assert_eq!(AppError::from(ErrorCode::NotFound).into_response().status(), StatusCode::NOT_FOUND);
		assert_eq!(AppError::from(ErrorCode::InsufficientPermissions).into_response().status(), StatusCode::FORBIDDEN);
		assert_eq!(AppError::from(ErrorCode::NotAuthenticated).into_response().status(), StatusCode::UNAUTHORIZED);
	}

	#[test]
	fn database_failure_is_a_server_error() {
		let response = AppError::from(sqlx::Error::PoolTimedOut).into_response();
		assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
	}

	#[test]
	fn gateway_errors_map_to_their_codes() {
		assert_eq!(AppError::from(GatewayKeyError::NotFound).code(), ErrorCode::ApiKeyNotFound);
		assert_eq!(AppError::from(GatewayAdminError::InvalidQuery).code(), ErrorCode::ValidationFailed);
		assert_eq!(AppError::from(GatewayAdminError::Database(sqlx::Error::PoolTimedOut)).code(), ErrorCode::DatabaseError);
	}

	#[test]
	fn non_database_error_is_not_a_unique_violation() {
		assert!(!crate::types::is_unique_violation(&sqlx::Error::RowNotFound));
	}
}
