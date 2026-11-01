use crate::config::OAuthProvider;
use crate::types::Role;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "Base"))]
pub struct BaseResponse {
	#[cfg_attr(test, ts(type = "any"))]
	pub i18n: Arc<Value>,
	pub language: String,
	pub needs_setup: bool,
	pub oauth_providers: Vec<OAuthProvider>,
	pub roles: Vec<Role>,
	/// Whether the chat upstream-provider selector is enabled instance-wide.
	pub enable_provider_selector: bool,
	/// Whether admins may register server-side stdio MCP servers.
	pub allow_server_stdio_mcp: bool,
	/// Instance-wide default model ID.
	pub default_model_id: Option<Uuid>,
}
