use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CostDetails, Message, ReasoningDetails, RequestSettings, ThemeCssVars, ToolExecutionResponse, UsageDetails, UserPreferences};

#[derive(Debug, Serialize)]
pub struct WorkspaceResponse {
	pub id: Uuid,
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: i32,
	pub is_default: bool,
	pub chat_count: i64,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl WorkspaceResponse {
	pub fn from_workspace(ws: super::Workspace, chat_count: i64) -> Self {
		Self {
			id: ws.id,
			name: ws.name,
			icon: ws.icon,
			color: ws.color,
			sort_order: ws.sort_order,
			is_default: ws.is_default,
			chat_count,
			created_at: ws.created_at,
			updated_at: ws.updated_at,
		}
	}
}

impl From<super::rows::WorkspaceWithCount> for WorkspaceResponse {
	fn from(ws: super::rows::WorkspaceWithCount) -> Self {
		Self {
			id: ws.id,
			name: ws.name,
			icon: ws.icon,
			color: ws.color,
			sort_order: ws.sort_order,
			is_default: ws.is_default,
			chat_count: ws.chat_count,
			created_at: ws.created_at,
			updated_at: ws.updated_at,
		}
	}
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
	pub id: Uuid,
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
	pub is_pinned: bool,
	pub is_archived: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub branched_from_chat_id: Option<Uuid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub branched_from_message_id: Option<Uuid>,
	pub message_count: i64,
	pub last_message_at: Option<DateTime<Utc>>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ChatWithMessagesResponse {
	pub chat: ChatResponse,
	pub messages: Vec<ChatMessageResponse>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatMessageResponse {
	pub id: Uuid,
	pub role: String,
	pub content: String,
	pub reasoning_content: Option<String>,
	pub model_id: Option<Uuid>,
	pub model_key: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub content_parts: Option<serde_json::Value>,
	pub cost_details: CostDetails,
	pub usage_details: UsageDetails,
	pub reasoning_details: ReasoningDetails,
	pub request_settings: RequestSettings,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_calls: Option<Vec<ToolExecutionResponse>>,
	pub created_at: DateTime<Utc>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub parent_id: Option<Uuid>,
	pub fork_index: i32,
	pub sibling_count: i32,
}

impl From<Message> for ChatMessageResponse {
	fn from(m: Message) -> Self {
		let request_settings = m.request_settings.0;
		let model_key = request_settings.model_key.clone();
		Self {
			id: m.id,
			role: m.role,
			content: m.content,
			reasoning_content: m.reasoning_content,
			model_id: m.model_id,
			model_key,
			content_parts: m.content_parts,
			cost_details: m.cost_details.0,
			usage_details: m.usage_details.0,
			reasoning_details: m.reasoning_details.0,
			request_settings,
			tool_calls: None,
			created_at: m.created_at,
			parent_id: m.parent_id,
			fork_index: m.fork_index,
			sibling_count: 1,
		}
	}
}

#[derive(Debug, Serialize)]
pub struct PreferencesResponse {
	pub default_model_key: Option<String>,
	pub effective_default_model_key: Option<String>,
	pub default_provider_slug: Option<String>,
	pub default_tools: Vec<String>,
	pub favorite_model_keys: Vec<String>,
	pub streaming_animation: String,
	pub use_remend: bool,
	pub theme_css_vars: ThemeCssVars,
	pub custom_theme_urls: Vec<String>,
}

impl From<UserPreferences> for PreferencesResponse {
	fn from(p: UserPreferences) -> Self {
		Self {
			default_model_key: p.default_model_key,
			effective_default_model_key: None,
			default_provider_slug: p.default_provider_slug,
			default_tools: serde_json::from_value(p.default_tools).unwrap_or_default(),
			favorite_model_keys: serde_json::from_value(p.favorite_model_keys).unwrap_or_default(),
			streaming_animation: p.streaming_animation,
			use_remend: p.use_remend,
			theme_css_vars: serde_json::from_value(p.theme_css_vars).unwrap_or_default(),
			custom_theme_urls: serde_json::from_value(p.custom_theme_urls).unwrap_or_default(),
		}
	}
}

impl Default for PreferencesResponse {
	fn default() -> Self {
		Self {
			default_model_key: None,
			effective_default_model_key: None,
			default_provider_slug: None,
			default_tools: Vec::new(),
			favorite_model_keys: Vec::new(),
			streaming_animation: "fade".to_string(),
			use_remend: true,
			theme_css_vars: ThemeCssVars::default(),
			custom_theme_urls: Vec::new(),
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfigResponse {
	pub default_theme: ThemeCssVars,
	pub enable_provider_selector: bool,
	pub allow_server_stdio_mcp: bool,
	pub default_model_key: Option<String>,
}

impl Default for GlobalConfigResponse {
	fn default() -> Self {
		Self {
			default_theme: ThemeCssVars::default(),
			enable_provider_selector: false,
			allow_server_stdio_mcp: false,
			default_model_key: None,
		}
	}
}

#[derive(Debug, Serialize)]
pub struct BranchResponse {
	pub chat: ChatResponse,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub prefill_content: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub prefill_parts: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamData {
	UserMessageSaved {
		message: ChatMessageResponse,
	},
	TextDelta {
		content: String,
	},
	ReasoningDelta {
		content: String,
	},
	ToolCallStart {
		id: String,
		name: String,
	},
	ToolCallDelta {
		id: String,
		args_delta: String,
	},
	ToolCallEnd {
		id: String,
	},
	ToolResult {
		id: String,
		output: serde_json::Value,
		error: Option<String>,
		tool_id: Option<Uuid>,
		tool_function: Option<Uuid>,
		tool_name: Option<String>,
	},
	/// Emitted when the AI calls a user-owned MCP tool that must run client-side.
	/// The browser executes the tool against the local MCP server and POSTs the
	/// result back via the tool-result endpoint before the stream can continue.
	ClientToolCall {
		id: String,
		name: String,
		args: serde_json::Value,
		mcp_server_id: Uuid,
		mcp_tool_name: String,
	},
	Tokens {
		input: u32,
		output: u32,
		reasoning: Option<u32>,
	},
	Usage {
		cost_details: CostDetails,
	},
	Error {
		code: String,
		message: String,
	},
	Done {
		message: ChatMessageResponse,
	},
}
