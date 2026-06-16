use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
	pub code: String,
	pub state: String,
}
