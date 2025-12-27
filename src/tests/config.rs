#[cfg(test)]
mod tests {
	use crate::config::{ConfigValues, OAuthProvider};
	use crate::i18n::Language;

	#[test]
	fn test_config_values_default() {
		let values = ConfigValues::default();
		assert_eq!(values.language, Language::En);
	}

	#[test]
	fn test_oauth_provider_from_str() {
		assert_eq!(OAuthProvider::from_str("google"), Some(OAuthProvider::Google));
		assert_eq!(OAuthProvider::from_str("Google"), Some(OAuthProvider::Google));
		assert_eq!(OAuthProvider::from_str("apple"), Some(OAuthProvider::Apple));
		assert_eq!(OAuthProvider::from_str("Apple"), Some(OAuthProvider::Apple));
		assert_eq!(OAuthProvider::from_str("discord"), Some(OAuthProvider::Discord));
		assert_eq!(OAuthProvider::from_str("Discord"), Some(OAuthProvider::Discord));
		assert_eq!(OAuthProvider::from_str("unknown"), None);
	}

	#[test]
	fn test_oauth_provider_as_str() {
		assert_eq!(OAuthProvider::Google.as_str(), "google");
		assert_eq!(OAuthProvider::Apple.as_str(), "apple");
		assert_eq!(OAuthProvider::Discord.as_str(), "discord");
	}

	#[test]
	fn test_oauth_provider_all() {
		let all = OAuthProvider::all();
		assert_eq!(all.len(), 3);
		assert!(all.contains(&OAuthProvider::Google));
		assert!(all.contains(&OAuthProvider::Apple));
		assert!(all.contains(&OAuthProvider::Discord));
	}
}
