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
	pub email_verified: Option<bool>,
	pub username: Option<String>,
	pub raw_data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
	#[serde(alias = "id")]
	pub sub: String,
	pub email: String,
	#[serde(alias = "verified_email")]
	pub email_verified: Option<bool>,
	pub name: Option<String>,
}

impl From<GoogleUserInfo> for OAuthUserInfo {
	fn from(g: GoogleUserInfo) -> Self {
		Self {
			provider: "google".to_string(),
			provider_user_id: g.sub.clone(),
			email: Some(g.email),
			email_verified: g.email_verified,
			username: g.name,
			raw_data: serde_json::json!({ "sub": g.sub }),
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct DiscordUserInfo {
	pub id: String,
	pub email: String,
	pub verified: Option<bool>,
	pub username: String,
}

impl From<DiscordUserInfo> for OAuthUserInfo {
	fn from(d: DiscordUserInfo) -> Self {
		Self {
			provider: "discord".to_string(),
			provider_user_id: d.id.clone(),
			email: Some(d.email),
			email_verified: d.verified,
			username: Some(d.username),
			raw_data: serde_json::json!({ "id": d.id }),
		}
	}
}
