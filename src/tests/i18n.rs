#[cfg(test)]
mod tests {

	mod types {
		use crate::types::{Translation, UpsertTranslationRequest};

		#[test]
		fn upsert_request_deserializes_with_defaults() {
			let json = r#"{"language": "en", "key_path": "test.key", "value": "Hello"}"#;
			let req: UpsertTranslationRequest = serde_json::from_str(json).unwrap();
			assert_eq!(req.language, "en");
			assert_eq!(req.key_path, "test.key");
			assert_eq!(req.value, "Hello");
			assert!(!req.is_override); // default
		}

		#[test]
		fn upsert_request_deserializes_with_override() {
			let json = r#"{"language": "de", "key_path": "test.key", "value": "Hallo", "is_override": true}"#;
			let req: UpsertTranslationRequest = serde_json::from_str(json).unwrap();

			assert!(req.is_override);
		}

		#[test]
		fn translation_serializes_correctly() {
			let translation = Translation {
				id: sqlx::types::Uuid::nil(),
				language: "en".to_string(),
				key_path: "test.key".to_string(),
				value: "Hello".to_string(),
				is_override: false,
			};

			let json = serde_json::to_string(&translation).unwrap();
			assert!(json.contains("\"language\":\"en\""));
			assert!(json.contains("\"is_override\":false"));
		}
	}
}
