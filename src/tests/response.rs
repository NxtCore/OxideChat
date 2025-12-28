#[cfg(test)]
mod tests {
	use axum::http::StatusCode;

	use crate::utils::response::{ErrorCode, ErrorDetail};

	#[test]
	fn error_code_status_mapping() {
		assert_eq!(ErrorCode::BadRequest.status(), StatusCode::BAD_REQUEST);
		assert_eq!(ErrorCode::Unauthorized.status(), StatusCode::UNAUTHORIZED);
		assert_eq!(ErrorCode::Forbidden.status(), StatusCode::FORBIDDEN);
		assert_eq!(ErrorCode::NotFound.status(), StatusCode::NOT_FOUND);
		assert_eq!(ErrorCode::Conflict.status(), StatusCode::CONFLICT);
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
