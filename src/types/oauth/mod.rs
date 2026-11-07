use serde::{Deserialize, Serialize};

mod claims;
mod provider;
mod requests;
mod sign_in;

pub use claims::*;
pub use provider::*;
pub use requests::*;
pub use sign_in::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
	pub csrf_token: String,
	pub pkce_verifier: String,
	pub provider_slug: String,
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
