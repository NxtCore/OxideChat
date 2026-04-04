//! Chat system types for OxideChat API.
//!
//! Types for workspaces, chats, messages, and user preferences.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StreamingAnimation {
	#[default]
	Fade,
	Typewriter,
	Slide,
	None,
}

impl StreamingAnimation {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Fade => "fade",
			Self::Typewriter => "typewriter",
			Self::Slide => "slide",
			Self::None => "none",
		}
	}

	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s.to_lowercase().as_str() {
			"fade" => Some(Self::Fade),
			"typewriter" => Some(Self::Typewriter),
			"slide" => Some(Self::Slide),
			"none" => Some(Self::None),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
	User,
	Assistant,
	System,
}

impl MessageRole {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::User => "user",
			Self::Assistant => "assistant",
			Self::System => "system",
		}
	}
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CostDetails {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub input: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub output: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub reasoning: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub total: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageDetails {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub input_tokens: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub output_tokens: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub reasoning_tokens: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub latency_ms: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub reasoning_latency_ms: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReasoningDetails {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub effort: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub budget_tokens: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResponse {
	pub tool_call_id: String,
	pub tool_name: String,
	pub input_args: serde_json::Value,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub output: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub execution_ms: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_id: Option<Uuid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_function: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionInternal {
	pub call_id: String,
	pub tool_name: String,
	pub args: serde_json::Value,
	pub output: serde_json::Value,
	pub error: Option<String>,
	pub execution_ms: i32,
	pub tool_id: Option<Uuid>,
	pub function_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeCssVars {
	#[serde(default)]
	pub theme: std::collections::HashMap<String, String>,
	#[serde(default)]
	pub light: std::collections::HashMap<String, String>,
	#[serde(default)]
	pub dark: std::collections::HashMap<String, String>,
}

// ============= Internal Types =============

#[derive(Debug, Clone, FromRow)]
pub struct Workspace {
	pub id: Uuid,
	pub user_id: Uuid,
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: i32,
	pub is_default: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceWithCount {
	pub id: Uuid,
	pub user_id: Uuid,
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: i32,
	pub is_default: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub chat_count: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Chat {
	pub id: Uuid,
	pub user_id: Uuid,
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
	pub is_pinned: bool,
	pub is_archived: bool,
	pub branched_from_chat_id: Option<Uuid>,
	pub branched_from_message_id: Option<Uuid>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Message {
	pub id: Uuid,
	pub chat_id: Uuid,
	pub role: String,
	pub content: String,
	pub content_parts: Option<serde_json::Value>,
	pub reasoning_content: Option<String>,
	pub model_id: Option<Uuid>,
	pub cost_details: sqlx::types::Json<CostDetails>,
	pub usage_details: sqlx::types::Json<UsageDetails>,
	pub reasoning_details: sqlx::types::Json<ReasoningDetails>,
	pub created_at: DateTime<Utc>,
	// Fork support
	pub parent_id: Option<Uuid>,
	pub fork_index: i32,
	pub is_active_fork: bool,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserPreferences {
	pub user_id: Uuid,
	pub default_model_key: Option<String>,
	pub favorite_model_keys: serde_json::Value,
	pub streaming_animation: String,
	pub use_remend: bool,
	pub theme_css_vars: serde_json::Value,
	pub custom_theme_urls: serde_json::Value,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl UserPreferences {
	#[must_use]
	pub fn default_for(user_id: Uuid) -> Self {
		let now = Utc::now();
		Self {
			user_id,
			default_model_key: None,
			favorite_model_keys: serde_json::json!([]),
			streaming_animation: "fade".to_string(),
			use_remend: true,
			theme_css_vars: serde_json::json!({}),
			custom_theme_urls: serde_json::json!([]),
			created_at: now,
			updated_at: now,
		}
	}
}

// ============= Request Types =============

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
	pub regenerate: bool, // Whether to trigger AI regeneration after editing
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

/// Structured message part (text or image)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessagePart {
	Text { text: String },
	Image { image_id: String },
}

/// Request body for sending a message and streaming AI response
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
	/// If true, skip creating a new user message and use existing messages for regeneration
	#[serde(default)]
	pub skip_user_message: bool,
	/// If set, regenerate from this assistant message (creates a new fork sibling)
	pub regenerate_from_message_id: Option<String>,
}

// ============= Response Types =============

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
	pub fn from_workspace(ws: Workspace, chat_count: i64) -> Self {
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

impl From<WorkspaceWithCount> for WorkspaceResponse {
	fn from(ws: WorkspaceWithCount) -> Self {
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
	#[serde(skip_serializing_if = "Option::is_none")]
	pub content_parts: Option<serde_json::Value>,
	pub cost_details: CostDetails,
	pub usage_details: UsageDetails,
	pub reasoning_details: ReasoningDetails,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_calls: Option<Vec<ToolExecutionResponse>>,
	pub created_at: DateTime<Utc>,
	// Fork support
	#[serde(skip_serializing_if = "Option::is_none")]
	pub parent_id: Option<Uuid>,
	pub fork_index: i32,
	pub sibling_count: i32,
}

impl From<Message> for ChatMessageResponse {
	fn from(m: Message) -> Self {
		Self {
			id: m.id,
			role: m.role,
			content: m.content,
			reasoning_content: m.reasoning_content,
			model_id: m.model_id,
			content_parts: m.content_parts,
			cost_details: m.cost_details.0,
			usage_details: m.usage_details.0,
			reasoning_details: m.reasoning_details.0,
			tool_calls: None,
			created_at: m.created_at,
			parent_id: m.parent_id,
			fork_index: m.fork_index,
			sibling_count: 1, // Default, computed at query time
		}
	}
}

#[derive(Debug, Serialize)]
pub struct PreferencesResponse {
	pub default_model_key: Option<String>,
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
}

impl Default for GlobalConfigResponse {
	fn default() -> Self {
		Self {
			default_theme: ThemeCssVars::default(),
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

/// SSE event data
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamData {
	/// User message saved confirmation
	UserMessageSaved { message: ChatMessageResponse },
	/// Text content delta
	TextDelta { content: String },
	/// Reasoning text delta (for models that support it)
	ReasoningDelta { content: String },
	/// Tool call started
	ToolCallStart { id: String, name: String },
	/// Tool call argument delta
	ToolCallDelta { id: String, args_delta: String },
	/// Tool call ended (arguments complete)
	ToolCallEnd { id: String },
	/// Tool execution result
	ToolResult {
		id: String,
		output: serde_json::Value,
		error: Option<String>,
		tool_id: Option<Uuid>,
		tool_function: Option<Uuid>,
		tool_name: Option<String>,
	},
	/// Token count update
	Tokens { input: u32, output: u32, reasoning: Option<u32> },
	/// Usage details
	Usage { cost_details: CostDetails },
	/// Error occurred
	Error { code: String, message: String },
	/// Stream completed with message info
	Done { message: ChatMessageResponse },
}
