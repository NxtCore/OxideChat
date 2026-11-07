use serde::Deserialize;

/// What the provider sends back: a code, or an error when the person declined.
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
	pub code: Option<String>,
	pub state: String,
	pub error: Option<String>,
}
