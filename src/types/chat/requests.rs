use serde::{Deserialize, Deserializer};
use uuid::Uuid;

use super::ThemeCssVars;
use crate::config::OAuthProvider;
use crate::i18n::Language;

/// Distinguishes an absent field (`None`) from an explicit JSON `null` (`Some(None)`),
/// so a PATCH can clear a nullable column instead of leaving it unchanged.
pub(crate) fn deserialize_nullable_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
	T: Deserialize<'de>,
	D: Deserializer<'de>,
{
	Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct CreateWorkspaceRequest {
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	#[serde(default)]
	pub is_default: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct UpdateWorkspaceRequest {
	pub name: Option<String>,
	pub icon: Option<String>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub color: Option<Option<String>>,
	pub sort_order: Option<i32>,
	pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum WorkspaceDeleteAction {
	#[default]
	Archive,
	Move,
	Delete,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DeleteWorkspaceOptions", optional_fields = nullable))]
pub struct DeleteWorkspaceParams {
	#[serde(default)]
	pub action: WorkspaceDeleteAction,
	pub target_workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct CreateChatRequest {
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct UpdateChatRequest {
	pub title: Option<String>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub workspace_id: Option<Option<Uuid>>,
	pub is_pinned: Option<bool>,
	pub is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatListParams {
	pub workspace_id: Option<Uuid>,
	#[serde(default)]
	pub include_archived: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct SendMessageRequest {
	pub content: String,
	pub model_id: Option<Uuid>,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct MessageListParams {
	pub before: Option<Uuid>,
	pub after: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditMessageRequest {
	pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct SwitchForkRequest {
	pub fork_index: i32,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct UpdatePreferencesRequest {
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub default_model_key: Option<Option<String>>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub default_provider_slug: Option<Option<String>>,
	pub default_tools: Option<Vec<String>>,
	pub favorite_model_keys: Option<Vec<String>>,
	pub streaming_animation: Option<String>,
	pub use_remend: Option<bool>,
	pub theme_css_vars: Option<ThemeCssVars>,
	pub custom_theme_urls: Option<Vec<String>>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub language: Option<Option<Language>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGlobalConfigRequest {
	pub default_theme: Option<ThemeCssVars>,
	pub enable_provider_selector: Option<bool>,
	pub allow_server_stdio_mcp: Option<bool>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub default_model_id: Option<Option<Uuid>>,
	pub language: Option<Language>,
	#[serde(default)]
	pub oauth: Vec<UpdateOAuthProviderRequest>,
}

/// Omitted fields stay unchanged; `null` removes the stored value.
#[derive(Debug, Deserialize)]
pub struct UpdateOAuthProviderRequest {
	pub provider: OAuthProvider,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub client_id: Option<Option<String>>,
	#[serde(default, deserialize_with = "deserialize_nullable_field")]
	pub client_secret: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ClientToolResultRequest {
	pub call_id: String,
	pub result: serde_json::Value,
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
	Reasoning { text: String },
	Image { image_id: String },
	#[serde(rename = "tool_call")]
	ToolCall { id: String, name: String, arguments: String },
	#[serde(rename = "tool_result")]
	ToolResult { tool_call_id: String },
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
	/// Upstream-provider slug the user picked in the chat provider selector (`OpenRouter` only).
	/// Empty/None means automatic routing.
	pub provider_slug: Option<String>,
	/// How the picked provider is applied: `"prefer"` (order, fallbacks on) or `"lock"`
	/// (only, no fallback). Defaults to `"prefer"`.
	pub provider_routing_mode: Option<String>,
}

const MAX_CUSTOM_SAMPLING_FIELDS: usize = 32;
const MAX_CUSTOM_SAMPLING_KEY_LEN: usize = 64;
const MAX_CUSTOM_SAMPLING_BYTES: usize = 16 * 1024;

/// Sampling options saved on a chat. Unset fields fall back to the model's defaults, and
/// `custom` holds extra provider request fields for experienced users.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct ChatSampling {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_p: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub frequency_penalty: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub presence_penalty: Option<f32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub seed: Option<i64>,
	#[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
	#[cfg_attr(test, ts(optional, type = "Record<string, unknown>"))]
	pub custom: serde_json::Map<String, serde_json::Value>,
}

fn in_range(value: Option<f32>, min: f32, max: f32) -> bool {
	value.is_none_or(|v| v.is_finite() && (min..=max).contains(&v))
}

fn valid_custom_key(key: &str) -> bool {
	!key.is_empty()
		&& key.len() <= MAX_CUSTOM_SAMPLING_KEY_LEN
		&& key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
		&& !omniference::adapter::is_reserved_extra_body_key(key)
}

impl ChatSampling {
	/// Returns `true` when every value is in range and the custom fields are allowed.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		in_range(self.temperature, 0.0, 2.0)
			&& in_range(self.top_p, 0.0, 1.0)
			&& in_range(self.frequency_penalty, -2.0, 2.0)
			&& in_range(self.presence_penalty, -2.0, 2.0)
			&& self.max_tokens.is_none_or(|t| t > 0)
			&& self.custom.len() <= MAX_CUSTOM_SAMPLING_FIELDS
			&& self.custom.keys().all(|key| valid_custom_key(key))
			&& serde_json::to_vec(&self.custom).is_ok_and(|bytes| bytes.len() <= MAX_CUSTOM_SAMPLING_BYTES)
	}

	/// Converts the typed fields into Omniference sampling options.
	#[must_use]
	pub fn to_sampling(&self) -> omniference::Sampling {
		omniference::Sampling {
			temperature: self.temperature,
			top_p: self.top_p,
			max_tokens: self.max_tokens,
			frequency_penalty: self.frequency_penalty,
			presence_penalty: self.presence_penalty,
			seed: self.seed,
			..Default::default()
		}
	}
}
