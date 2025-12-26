#[cfg(test)]
mod tests {
	use crate::types::BaseResponse;

	#[test]
	fn base_response_serializes() {
		let response = BaseResponse {
			i18n: serde_json::json!({"en": {"hello": "world"}}),
		};

		let json = serde_json::to_string(&response).unwrap();
		assert!(json.contains("\"i18n\""));
	}
}
