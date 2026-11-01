#[cfg(test)]
mod tests {
	use crate::config::{ConfigValues, OAuthProvider};
	use crate::i18n::Language;
	use crate::types::{OAuthSettingsResponse, UpdateGlobalConfigRequest};

	#[test]
	fn test_config_values_default() {
		let values = ConfigValues::default();
		assert_eq!(values.language, Language::En);
	}

	#[test]
	fn test_oauth_provider_from_str() {
		assert_eq!(OAuthProvider::from_str("google"), Some(OAuthProvider::Google));
		assert_eq!(OAuthProvider::from_str("Google"), Some(OAuthProvider::Google));
		assert_eq!(OAuthProvider::from_str("discord"), Some(OAuthProvider::Discord));
		assert_eq!(OAuthProvider::from_str("Discord"), Some(OAuthProvider::Discord));
		assert_eq!(OAuthProvider::from_str("unknown"), None);
	}

	#[test]
	fn test_oauth_provider_as_str() {
		assert_eq!(OAuthProvider::Google.as_str(), "GOOGLE");
		assert_eq!(OAuthProvider::Discord.as_str(), "DISCORD");
	}

	#[test]
	fn test_oauth_provider_all() {
		let all = OAuthProvider::all();
		assert_eq!(all.len(), 2);
		assert!(all.contains(&OAuthProvider::Google));
		assert!(all.contains(&OAuthProvider::Discord));
	}

	#[test]
	fn update_request_reads_language_and_rejects_unknown_ones() {
		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"language":"de"}"#).unwrap();
		assert_eq!(request.language, Some(Language::De));
		assert!(request.oauth.is_empty());
		assert!(serde_json::from_str::<UpdateGlobalConfigRequest>(r#"{"language":"fr"}"#).is_err());
	}

	#[test]
	fn oauth_update_tells_omitted_from_null() {
		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"oauth":[{"provider":"GOOGLE","client_id":"id","client_secret":null}]}"#).unwrap();
		let update = &request.oauth[0];
		assert_eq!(update.provider, OAuthProvider::Google);
		assert_eq!(update.client_id, Some(Some("id".to_string())));
		assert_eq!(update.client_secret, Some(None));

		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"oauth":[{"provider":"DISCORD"}]}"#).unwrap();
		assert_eq!(request.oauth[0].client_id, None);
		assert_eq!(request.oauth[0].client_secret, None);
	}

	#[test]
	fn oauth_settings_response_has_no_secret_field() {
		let json = serde_json::to_value(OAuthSettingsResponse {
			provider: OAuthProvider::Google,
			client_id: Some("id".to_string()),
			has_client_secret: true,
			env_configured: false,
			configured: true,
			callback_url: None,
		})
		.unwrap();
		assert_eq!(json["has_client_secret"], true);
		assert!(json.get("client_secret").is_none());
	}
}
