//! Chat system types for OxideChat API.
//!
//! Types for workspaces, chats, messages, and user preferences.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============= Enums =============

/// Streaming animation types for message rendering
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

/// Message role
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

// ============= Structs =============

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostDetails {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub input: Option<Decimal>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub output: Option<Decimal>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub reasoning: Option<Decimal>,
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

/// Tool execution result for API response
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

// ============= Database Models =============

/// Workspace database row
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

/// Workspace with chat count for list queries
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

/// Chat database row
#[derive(Debug, Clone, FromRow)]
pub struct Chat {
	pub id: Uuid,
	pub user_id: Uuid,
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
	pub is_pinned: bool,
	pub is_archived: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Message database row
#[derive(Debug, Clone, FromRow)]
pub struct Message {
	pub id: Uuid,
	pub chat_id: Uuid,
	pub role: String,
	pub content: String,
	pub reasoning_content: Option<String>,
	pub model_id: Option<Uuid>,
	pub cost_details: sqlx::types::Json<CostDetails>,
	pub usage_details: sqlx::types::Json<UsageDetails>,
	pub reasoning_details: sqlx::types::Json<ReasoningDetails>,
	pub created_at: DateTime<Utc>,
}

/// User preferences database row
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserPreferences {
	pub user_id: Uuid,
	pub default_model_key: Option<String>,
	pub favorite_model_keys: serde_json::Value,
	pub streaming_animation: String,
	pub use_remend: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl UserPreferences {
	/// Create default preferences for a user when none exist in database
	#[must_use]
	pub fn default_for(user_id: Uuid) -> Self {
		let now = Utc::now();
		Self {
			user_id,
			default_model_key: None,
			favorite_model_keys: serde_json::json!([]),
			streaming_animation: "fade".to_string(),
			use_remend: true,
			created_at: now,
			updated_at: now,
		}
	}
}

// ============= Request DTOs =============

/// Request to create a workspace
#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	#[serde(default)]
	pub is_default: bool,
}

/// Request to update a workspace
#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
	pub name: Option<String>,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: Option<i32>,
	pub is_default: Option<bool>,
}

/// Request to create a chat
#[derive(Debug, Deserialize)]
pub struct CreateChatRequest {
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
}

/// Request to update a chat
#[derive(Debug, Deserialize)]
pub struct UpdateChatRequest {
	pub title: Option<String>,
	pub workspace_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub is_pinned: Option<bool>,
	pub is_archived: Option<bool>,
}

/// Chat list filter parameters
#[derive(Debug, Deserialize)]
pub struct ChatListParams {
	pub workspace_id: Option<Uuid>,
	#[serde(default)]
	pub include_archived: bool,
	pub limit: Option<i32>,
	pub offset: Option<i32>,
}

/// Request to send a message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
	pub content: String,
	pub model_id: Option<Uuid>,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<i32>,
	#[serde(default)]
	pub tools_enabled: Vec<String>,
}

/// Message list pagination
#[derive(Debug, Deserialize)]
pub struct MessageListParams {
	pub limit: Option<i32>,
	pub before: Option<Uuid>,
	pub after: Option<Uuid>,
}

/// Request to update user preferences
#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
	pub default_model_key: Option<String>,
	pub favorite_model_keys: Option<Vec<String>>,
	pub streaming_animation: Option<String>,
	pub use_remend: Option<bool>,
}

// ============= Response DTOs =============

/// Workspace response
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

/// Chat response (list view)
#[derive(Debug, Serialize)]
pub struct ChatResponse {
	pub id: Uuid,
	pub workspace_id: Option<Uuid>,
	pub title: Option<String>,
	pub is_pinned: bool,
	pub is_archived: bool,
	pub message_count: i64,
	pub last_message_at: Option<DateTime<Utc>>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Chat with messages (detail view)
#[derive(Debug, Serialize)]
pub struct ChatWithMessagesResponse {
	pub chat: ChatResponse,
	pub messages: Vec<ChatMessageResponse>,
}

/// Chat message response
#[derive(Debug, Serialize, Clone)]
pub struct ChatMessageResponse {
	pub id: Uuid,
	pub role: String,
	pub content: String,
	pub reasoning_content: Option<String>,
	pub model_id: Option<Uuid>,
	pub cost_details: CostDetails,
	pub usage_details: UsageDetails,
	pub reasoning_details: ReasoningDetails,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_calls: Option<Vec<ToolExecutionResponse>>,
	pub created_at: DateTime<Utc>,
}

impl From<Message> for ChatMessageResponse {
	fn from(m: Message) -> Self {
		Self {
			id: m.id,
			role: m.role,
			content: m.content,
			reasoning_content: m.reasoning_content,
			model_id: m.model_id,
			cost_details: m.cost_details.0,
			usage_details: m.usage_details.0,
			reasoning_details: m.reasoning_details.0,
			tool_calls: None,
			created_at: m.created_at,
		}
	}
}

/// User preferences response
#[derive(Debug, Serialize)]
pub struct PreferencesResponse {
	pub default_model_key: Option<String>,
	pub favorite_model_keys: Vec<String>,
	pub streaming_animation: String,
	pub use_remend: bool,
}

impl From<UserPreferences> for PreferencesResponse {
	fn from(p: UserPreferences) -> Self {
		Self {
			default_model_key: p.default_model_key,
			favorite_model_keys: serde_json::from_value(p.favorite_model_keys).unwrap_or_default(),
			streaming_animation: p.streaming_animation,
			use_remend: p.use_remend,
		}
	}
}

/// Default preferences when user has no saved preferences
impl Default for PreferencesResponse {
	fn default() -> Self {
		Self {
			default_model_key: None,
			favorite_model_keys: Vec::new(),
			streaming_animation: "fade".to_string(),
			use_remend: true,
		}
	}
}
