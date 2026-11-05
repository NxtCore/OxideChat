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
				language: String::from("en"),
				key_path: String::from("test.key"),
				value: String::from("Hello"),
				is_override: false,
				created_at: chrono::Utc::now(),
				updated_at: chrono::Utc::now(),
			};

			let json = serde_json::to_string(&translation).unwrap();
			assert!(json.contains("\"language\":\"en\""));
			assert!(json.contains("\"is_override\":false"));
		}
	}

	mod user_language {
		use crate::i18n::Language;
		use crate::tests::support::{TestResult, create_user};
		use crate::types::{UpdatePreferencesRequest, UserPreferences};
		use sqlx::PgPool;

		fn request(json: &str) -> serde_json::Result<UpdatePreferencesRequest> {
			serde_json::from_str(json)
		}

		#[tokio::test]
		async fn scope_sets_the_request_language() {
			assert_eq!(Language::De.scope(async { Language::current() }).await, Language::De);
		}

		#[sqlx::test]
		async fn language_is_stored_kept_and_cleared(pool: PgPool) -> TestResult {
			let user_id = create_user(&pool).await?;
			sqlx::query("INSERT INTO user_preferences (user_id) VALUES ($1)").bind(user_id).execute(&pool).await?;
			assert_eq!(UserPreferences::language_for(&pool, &user_id).await?, None);

			UserPreferences::upsert(&pool, &user_id, &request(r#"{"language": "de"}"#)?).await?;
			assert_eq!(UserPreferences::language_for(&pool, &user_id).await?, Some(Language::De));

			UserPreferences::upsert(&pool, &user_id, &request(r#"{"use_remend": false}"#)?).await?;
			assert_eq!(UserPreferences::language_for(&pool, &user_id).await?, Some(Language::De));

			UserPreferences::upsert(&pool, &user_id, &request(r#"{"language": null}"#)?).await?;
			assert_eq!(UserPreferences::language_for(&pool, &user_id).await?, None);
			Ok(())
		}

		#[test]
		fn unknown_language_is_rejected() {
			assert!(request(r#"{"language": "fr"}"#).is_err());
		}
	}
}
