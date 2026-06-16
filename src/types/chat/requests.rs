use serde::Deserialize;
use uuid::Uuid;

use super::ThemeCssVars;

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	#[serde(default)]
	pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
	pub name: Option<String>,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: Option<i32>,
	pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChatRequest {
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChatRequest {
	pub title: Option<String>,
	pub workspace_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub is_pinned: Option<bool>,
	pub is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatListParams {
	pub workspace_id: Option<Uuid>,
	#[serde(default)]
	pub include_archived: bool,
	pub limit: Option<i32>,
	pub offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
	pub content: String,
	pub model_id: Option<Uuid>,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<i32>,
	#[serde(default)]
	pub tools_enabled: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageListParams {
	pub limit: Option<i32>,
	pub before: Option<Uuid>,
	pub after: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditMessageRequest {
	pub content: String,
	#[serde(default)]
	pub regenerate: bool,
}

#[derive(Debug, Deserialize)]
pub struct SwitchForkRequest {
	pub fork_index: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
	pub default_model_key: Option<String>,
	pub favorite_model_keys: Option<Vec<String>>,
	pub streaming_animation: Option<String>,
	pub use_remend: Option<bool>,
	pub theme_css_vars: Option<ThemeCssVars>,
	pub custom_theme_urls: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGlobalConfigRequest {
	pub default_theme: Option<ThemeCssVars>,
}

#[derive(Debug, Deserialize)]
pub struct BranchFromMessageRequest {
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessagePart {
	Text { text: String },
	Image { image_id: String },
}

#[derive(Debug, Deserialize)]
pub struct StreamRequest {
	pub content: String,
	#[serde(default)]
	pub parts: Option<Vec<MessagePart>>,
	pub model_key: String,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<u32>,
	pub tools_enabled: Option<Vec<String>>,
	pub sampling: Option<omniference::Sampling>,
	#[serde(default)]
	pub skip_user_message: bool,
	pub regenerate_from_message_id: Option<String>,
}
