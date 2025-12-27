#[cfg(test)]
mod tests {
	use crate::types::BaseResponse;

	#[test]
	fn base_response_serializes() {
		let response = BaseResponse {
			i18n: serde_json::json!({"en": {"hello": "world"}}),
			needs_setup: false,
		};

		let json = serde_json::to_string(&response).unwrap();
		assert!(json.contains("\"i18n\""));
		assert!(json.contains("\"needs_setup\""));
	}
}
