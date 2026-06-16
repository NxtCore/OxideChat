use serde::{Deserialize, Serialize};

mod requests;

pub use requests::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
	pub csrf_token: String,
	pub pkce_verifier: String,
	pub redirect_after: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthUserInfo {
	pub provider: String,
	pub provider_user_id: String,
	pub email: Option<String>,
	pub username: Option<String>,
	pub avatar_url: Option<String>,
	pub raw_data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
	pub id: String,
	pub email: String,
	pub name: Option<String>,
	pub picture: Option<String>,
}

impl From<GoogleUserInfo> for OAuthUserInfo {
	fn from(g: GoogleUserInfo) -> Self {
		Self {
			provider: "google".to_string(),
			provider_user_id: g.id.clone(),
			email: Some(g.email),
			username: g.name,
			avatar_url: g.picture,
			raw_data: serde_json::json!({ "id": g.id }),
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct DiscordUserInfo {
	pub id: String,
	pub email: String,
	pub username: String,
	pub avatar: Option<String>,
}

impl From<DiscordUserInfo> for OAuthUserInfo {
	fn from(d: DiscordUserInfo) -> Self {
		let avatar_url = d.avatar.as_ref().map(|hash| {
			let ext = if hash.starts_with("a_") { "gif" } else { "png" };
			format!("https://cdn.discordapp.com/avatars/{}/{}.{}", d.id, hash, ext)
		});
		Self {
			provider: "discord".to_string(),
			provider_user_id: d.id.clone(),
			email: Some(d.email),
			username: Some(d.username),
			avatar_url,
			raw_data: serde_json::json!({ "id": d.id }),
		}
	}
}
