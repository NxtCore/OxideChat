use crate::config::OAuthProvider;
use crate::types::Role;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct BaseResponse {
	pub i18n: Arc<Value>,
	pub language: String,
	pub needs_setup: bool,
	pub oauth_providers: Vec<OAuthProvider>,
	pub roles: Vec<Role>,
}
