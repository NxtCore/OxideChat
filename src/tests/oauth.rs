//! OAuth tests.
//!
//! Tests for OAuth provider user info mapping and email verification flags.

#[cfg(test)]
mod tests {
	mod google {
		use crate::types::oauth::{GoogleUserInfo, OAuthUserInfo};

		#[test]
		fn oidc_userinfo_maps_fields_and_verified_email() {
			let json = r#"{"sub":"123","email":"user@example.com","email_verified":true,"name":"User","picture":"https://pic"}"#;
			let info: GoogleUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.provider, "google");
			assert_eq!(user_info.provider_user_id, "123");
			assert_eq!(user_info.email.as_deref(), Some("user@example.com"));
			assert_eq!(user_info.email_verified, Some(true));
			assert_eq!(user_info.username.as_deref(), Some("User"));
			assert_eq!(user_info.avatar_url.as_deref(), Some("https://pic"));
		}

		#[test]
		fn unverified_email_is_reported() {
			let json = r#"{"sub":"123","email":"user@example.com","email_verified":false}"#;
			let info: GoogleUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.email_verified, Some(false));
		}

		#[test]
		fn missing_email_verified_is_none() {
			let json = r#"{"sub":"123","email":"user@example.com"}"#;
			let info: GoogleUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.email_verified, None);
		}

		#[test]
		fn legacy_v2_id_field_is_accepted_as_sub() {
			let json = r#"{"id":"123","email":"user@example.com","verified_email":true}"#;
			let info: GoogleUserInfo = serde_json::from_str(json).unwrap();

			assert_eq!(info.sub, "123");
		}
	}

	mod discord {
		use crate::types::oauth::{DiscordUserInfo, OAuthUserInfo};

		#[test]
		fn userinfo_maps_fields_and_verified_flag() {
			let json = r#"{"id":"42","email":"user@example.com","verified":true,"username":"user","avatar":null}"#;
			let info: DiscordUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.provider, "discord");
			assert_eq!(user_info.provider_user_id, "42");
			assert_eq!(user_info.email.as_deref(), Some("user@example.com"));
			assert_eq!(user_info.email_verified, Some(true));
			assert_eq!(user_info.username.as_deref(), Some("user"));
			assert_eq!(user_info.avatar_url, None);
		}

		#[test]
		fn unverified_email_is_reported() {
			let json = r#"{"id":"42","email":"user@example.com","verified":false,"username":"user"}"#;
			let info: DiscordUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.email_verified, Some(false));
		}

		#[test]
		fn missing_verified_is_none() {
			let json = r#"{"id":"42","email":"user@example.com","username":"user"}"#;
			let info: DiscordUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.email_verified, None);
		}

		#[test]
		fn animated_avatar_uses_gif_extension() {
			let json = r#"{"id":"42","email":"user@example.com","verified":true,"username":"user","avatar":"a_hash"}"#;
			let info: DiscordUserInfo = serde_json::from_str(json).unwrap();
			let user_info: OAuthUserInfo = info.into();

			assert_eq!(user_info.avatar_url.as_deref(), Some("https://cdn.discordapp.com/avatars/42/a_hash.gif"));
		}
	}
}
