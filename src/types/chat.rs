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
			streaming_animation: String::from("fade"),
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

impl crate::types::BaseType for Workspace {
	const TABLE: &'static str = "workspaces";
	const ALIAS: &'static str = "w";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			user_id: Uuid::new_v4(),
			name: String::new(),
			icon: None,
			color: None,
			sort_order: 0,
			is_default: false,
			created_at: Utc::now(),
			updated_at: Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id", "user_id", "name", "icon", "color",
			"sort_order", "is_default", "created_at", "updated_at",
		]
	}
}

impl Workspace {
	pub async fn list_by_user_with_count(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Vec<WorkspaceWithCount>, sqlx::Error> {
		sqlx::query_as::<_, WorkspaceWithCount>(
			r#"
			SELECT w.*, COALESCE(c.chat_count, 0) AS chat_count
			FROM workspaces w
			LEFT JOIN (
				SELECT workspace_id, COUNT(*) AS chat_count
				FROM chats
				WHERE user_id = $1
				GROUP BY workspace_id
			) c ON w.id = c.workspace_id
			WHERE w.user_id = $1
			ORDER BY w.sort_order, w.name
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	pub async fn find_by_id_and_user(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(pool: &sqlx::PgPool, user_id: &Uuid, name: &str, icon: Option<&str>, color: Option<&str>, is_default: bool) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Workspace>(
			r#"
			INSERT INTO workspaces (user_id, name, icon, color, is_default)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(name)
		.bind(icon)
		.bind(color)
		.bind(is_default)
		.fetch_one(pool)
		.await
	}

	pub async fn clear_default_for_user(pool: &sqlx::PgPool, user_id: &Uuid, exclude_id: Option<&Uuid>) -> Result<u64, sqlx::Error> {
		let result = if let Some(exclude) = exclude_id {
			sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1 AND id != $2")
				.bind(user_id)
				.bind(exclude)
				.execute(pool)
				.await?
		} else {
			sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1")
				.bind(user_id)
				.execute(pool)
				.await?
		};
		Ok(result.rows_affected())
	}

	pub async fn update(
		pool: &sqlx::PgPool,
		id: &Uuid,
		user_id: &Uuid,
		name: Option<&str>,
		icon: Option<&str>,
		color: Option<&str>,
		sort_order: Option<i32>,
		is_default: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Workspace>(
			r#"
			UPDATE workspaces
			SET name = COALESCE($3, name),
			    icon = COALESCE($4, icon),
			    color = COALESCE($5, color),
			    sort_order = COALESCE($6, sort_order),
			    is_default = COALESCE($7, is_default),
			    updated_at = NOW()
			WHERE id = $1 AND user_id = $2
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(user_id)
		.bind(name)
		.bind(icon)
		.bind(color)
		.bind(sort_order)
		.bind(is_default)
		.fetch_optional(pool)
		.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM workspaces WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}
}

impl crate::types::BaseType for Chat {
	const TABLE: &'static str = "chats";
	const ALIAS: &'static str = "c";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			user_id: Uuid::new_v4(),
			workspace_id: None,
			title: None,
			is_pinned: false,
			is_archived: false,
			branched_from_chat_id: None,
			branched_from_message_id: None,
			created_at: Utc::now(),
			updated_at: Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id", "user_id", "workspace_id", "title", "is_pinned", "is_archived",
			"branched_from_chat_id", "branched_from_message_id", "created_at", "updated_at",
		]
	}
}

impl Chat {
	pub async fn list_by_user(
		pool: &sqlx::PgPool,
		user_id: &Uuid,
		workspace_id: Option<&Uuid>,
		include_archived: bool,
		limit: i64,
		offset: i64,
	) -> Result<Vec<Self>, sqlx::Error> {
		if include_archived {
			if let Some(ws_id) = workspace_id {
				sqlx::query_as::<_, Chat>(
					"SELECT * FROM chats WHERE user_id = $1 AND workspace_id = $2 ORDER BY updated_at DESC LIMIT $3 OFFSET $4",
				)
				.bind(user_id)
				.bind(ws_id)
				.bind(limit)
				.bind(offset)
				.fetch_all(pool)
				.await
			} else {
				sqlx::query_as::<_, Chat>(
					"SELECT * FROM chats WHERE user_id = $1 ORDER BY updated_at DESC LIMIT $2 OFFSET $3",
				)
				.bind(user_id)
				.bind(limit)
				.bind(offset)
				.fetch_all(pool)
				.await
			}
		} else if let Some(ws_id) = workspace_id {
			sqlx::query_as::<_, Chat>(
				"SELECT * FROM chats WHERE user_id = $1 AND workspace_id = $2 AND is_archived = false ORDER BY updated_at DESC LIMIT $3 OFFSET $4",
			)
			.bind(user_id)
			.bind(ws_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(pool)
			.await
		} else {
			sqlx::query_as::<_, Chat>(
				"SELECT * FROM chats WHERE user_id = $1 AND is_archived = false ORDER BY updated_at DESC LIMIT $2 OFFSET $3",
			)
			.bind(user_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(pool)
			.await
		}
	}

	pub async fn find_by_id_and_user(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(pool: &sqlx::PgPool, user_id: &Uuid, workspace_id: Option<&Uuid>, title: Option<&str>) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Chat>(
			r#"
			INSERT INTO chats (user_id, workspace_id, title)
			VALUES ($1, $2, $3)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(workspace_id)
		.bind(title)
		.fetch_one(pool)
		.await
	}

	pub async fn update(
		pool: &sqlx::PgPool,
		id: &Uuid,
		user_id: &Uuid,
		title: Option<&str>,
		workspace_id: Option<&Uuid>,
		is_pinned: Option<bool>,
		is_archived: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Chat>(
			r#"
			UPDATE chats
			SET title = COALESCE($3, title),
			    workspace_id = COALESCE($4, workspace_id),
			    is_pinned = COALESCE($5, is_pinned),
			    is_archived = COALESCE($6, is_archived),
			    updated_at = NOW()
			WHERE id = $1 AND user_id = $2
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(user_id)
		.bind(title)
		.bind(workspace_id)
		.bind(is_pinned)
		.bind(is_archived)
		.fetch_optional(pool)
		.await
	}

	pub async fn touch(pool: &sqlx::PgPool, id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
			.bind(id)
			.execute(pool)
			.await?;
		Ok(())
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM chats WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn verify_workspace_belongs_to_user(pool: &sqlx::PgPool, workspace_id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let exists: Option<(i32,)> = sqlx::query_as(
			"SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2",
		)
		.bind(workspace_id)
		.bind(user_id)
		.fetch_optional(pool)
		.await?;
		Ok(exists.is_some())
	}

	pub async fn message_stats(pool: &sqlx::PgPool, chat_id: &Uuid) -> Result<(i64, Option<DateTime<Utc>>), sqlx::Error> {
		let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
			"SELECT COUNT(*), MAX(created_at) FROM messages WHERE chat_id = $1",
		)
		.bind(chat_id)
		.fetch_one(pool)
		.await?;
		Ok(row)
	}
}

impl crate::types::BaseType for Message {
	const TABLE: &'static str = "messages";
	const ALIAS: &'static str = "msg";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			chat_id: Uuid::new_v4(),
			role: String::from("user"),
			content: String::new(),
			content_parts: None,
			reasoning_content: None,
			model_id: None,
			cost_details: sqlx::types::Json(CostDetails::default()),
			usage_details: sqlx::types::Json(UsageDetails::default()),
			reasoning_details: sqlx::types::Json(ReasoningDetails::default()),
			parent_id: None,
			fork_index: 1,
			is_active_fork: true,
			created_at: Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id", "chat_id", "role", "content", "reasoning_content",
			"model_id", "parent_id", "fork_index", "is_active_fork",
			"created_at", "content_parts", "cost_details", "usage_details", "reasoning_details",
		]
	}
}

impl Message {
	pub async fn list_by_chat(
		pool: &sqlx::PgPool,
		chat_id: &Uuid,
		limit: Option<i32>,
		before: Option<&Uuid>,
		after: Option<&Uuid>,
	) -> Result<Vec<Self>, sqlx::Error> {
		if let Some(before_id) = before {
			let created_at: Option<DateTime<Utc>> = sqlx::query_scalar(
				"SELECT created_at FROM messages WHERE id = $1",
			)
			.bind(before_id)
			.fetch_optional(pool)
			.await?;
			if let Some(ts) = created_at {
				return sqlx::query_as::<_, Message>(
					"SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = true AND created_at < $2 ORDER BY created_at DESC LIMIT $3",
				)
				.bind(chat_id)
				.bind(ts)
				.bind(limit)
				.fetch_all(pool)
				.await;
			}
		}
		if let Some(after_id) = after {
			let created_at: Option<DateTime<Utc>> = sqlx::query_scalar(
				"SELECT created_at FROM messages WHERE id = $1",
			)
			.bind(after_id)
			.fetch_optional(pool)
			.await?;
			if let Some(ts) = created_at {
				return sqlx::query_as::<_, Message>(
					"SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = true AND created_at > $2 ORDER BY created_at ASC LIMIT $3",
				)
				.bind(chat_id)
				.bind(ts)
				.bind(limit)
				.fetch_all(pool)
				.await;
			}
		}
		sqlx::query_as::<_, Message>(
			"SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = true ORDER BY created_at ASC",
		)
		.bind(chat_id)
		.fetch_all(pool)
		.await
	}

	pub async fn list_active_by_chat(pool: &sqlx::PgPool, chat_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>(
			"SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = true ORDER BY created_at ASC",
		)
		.bind(chat_id)
		.fetch_all(pool)
		.await
	}

	pub async fn find_by_id_and_chat(pool: &sqlx::PgPool, id: &Uuid, chat_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
			.bind(id)
			.bind(chat_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		chat_id: &Uuid,
		role: &str,
		content: &str,
		reasoning_content: Option<&str>,
		model_id: Option<&Uuid>,
		parent_id: Option<&Uuid>,
		fork_index: i32,
		content_parts: Option<&serde_json::Value>,
		cost_details: Option<&CostDetails>,
		usage_details: Option<&UsageDetails>,
		reasoning_details: Option<&ReasoningDetails>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, reasoning_content, model_id,
			                     parent_id, fork_index, content_parts,
			                     cost_details, usage_details, reasoning_details)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
			RETURNING *
			"#,
		)
		.bind(chat_id)
		.bind(role)
		.bind(content)
		.bind(reasoning_content)
		.bind(model_id)
		.bind(parent_id)
		.bind(fork_index)
		.bind(content_parts)
		.bind(cost_details.map(sqlx::types::Json))
		.bind(usage_details.map(sqlx::types::Json))
		.bind(reasoning_details.map(sqlx::types::Json))
		.fetch_one(conn)
		.await
	}

	pub async fn next_fork_index(conn: &mut sqlx::PgConnection, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<i32, sqlx::Error> {
		let row: (Option<i32>,) = sqlx::query_as(
			"SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2",
		)
		.bind(chat_id)
		.bind(parent_id)
		.fetch_one(conn)
		.await?;
		Ok(row.0.unwrap_or(1))
	}

	pub async fn deactivate_subtree(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, message_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query(
			r#"
			WITH RECURSIVE descendants AS (
				SELECT id FROM messages WHERE id = $1
				UNION
				SELECT m.id FROM messages m INNER JOIN descendants d ON m.parent_id = d.id
			)
			UPDATE messages SET is_active_fork = false WHERE id IN (SELECT id FROM descendants)
			"#,
		)
		.bind(message_id)
		.execute(&mut **tx)
		.await?;
		Ok(result.rows_affected())
	}

	pub async fn activate_subtree(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, message_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query(
			r#"
			WITH RECURSIVE descendants AS (
				SELECT id FROM messages WHERE id = $1
				UNION
				SELECT m.id FROM messages m INNER JOIN descendants d ON m.parent_id = d.id
			)
			UPDATE messages SET is_active_fork = true WHERE id IN (SELECT id FROM descendants)
			"#,
		)
		.bind(message_id)
		.execute(&mut **tx)
		.await?;
		Ok(result.rows_affected())
	}

	pub async fn sibling_count(pool: &sqlx::PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>, role: &str) -> Result<i64, sqlx::Error> {
		let row: (i64,) = sqlx::query_as(
			"SELECT COUNT(*) FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND role = $3",
		)
		.bind(chat_id)
		.bind(parent_id)
		.bind(role)
		.fetch_one(pool)
		.await?;
		Ok(row.0)
	}

	pub async fn count_by_chat_and_parent(pool: &sqlx::PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<i64, sqlx::Error> {
		let row: (i64,) = sqlx::query_as(
			"SELECT COUNT(*) FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2",
		)
		.bind(chat_id)
		.bind(parent_id)
		.fetch_one(pool)
		.await?;
		Ok(row.0)
	}

	pub async fn siblings(
		pool: &sqlx::PgPool,
		chat_id: &Uuid,
		parent_id: Option<&Uuid>,
	) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>(
			"SELECT * FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 ORDER BY fork_index",
		)
		.bind(chat_id)
		.bind(parent_id)
		.fetch_all(pool)
		.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid, chat_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM messages WHERE id = $1 AND chat_id = $2")
			.bind(id)
			.bind(chat_id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}
}

impl crate::types::BaseType for UserPreferences {
	const TABLE: &'static str = "user_preferences";
	const ALIAS: &'static str = "up";

	fn new() -> Self {
		Self::default_for(Uuid::new_v4())
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"user_id", "default_model_key", "favorite_model_keys",
			"streaming_animation", "use_remend", "theme_css_vars",
			"custom_theme_urls", "created_at", "updated_at",
		]
	}
}

impl UserPreferences {
	pub async fn find_by_user_id(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserPreferences>("SELECT * FROM user_preferences WHERE user_id = $1")
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn upsert(pool: &sqlx::PgPool, user_id: &Uuid, prefs: &UpdatePreferencesRequest) -> Result<Self, sqlx::Error> {
		fn json_or_null<T: serde::Serialize>(v: &Option<T>) -> serde_json::Value {
			v.as_ref().map(|x| serde_json::to_value(x).unwrap_or(serde_json::Value::Null)).unwrap_or(serde_json::Value::Null)
		}

		sqlx::query_as::<_, UserPreferences>(
			r#"
			INSERT INTO user_preferences (user_id, default_model_key, favorite_model_keys,
			                              streaming_animation, use_remend, theme_css_vars, custom_theme_urls)
			VALUES ($1, $2, $3, $4, $5, $6, $7)
			ON CONFLICT (user_id) DO UPDATE
				SET default_model_key = COALESCE($2, user_preferences.default_model_key),
				    favorite_model_keys = CASE WHEN $3::jsonb = 'null'::jsonb THEN user_preferences.favorite_model_keys ELSE $3::jsonb END,
				    streaming_animation = COALESCE($4, user_preferences.streaming_animation),
				    use_remend = COALESCE($5, user_preferences.use_remend),
				    theme_css_vars = CASE WHEN $6::jsonb = 'null'::jsonb THEN user_preferences.theme_css_vars ELSE $6::jsonb END,
				    custom_theme_urls = CASE WHEN $7::jsonb = 'null'::jsonb THEN user_preferences.custom_theme_urls ELSE $7::jsonb END,
				    updated_at = NOW()
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(&prefs.default_model_key)
		.bind(json_or_null(&prefs.favorite_model_keys))
		.bind(&prefs.streaming_animation)
		.bind(prefs.use_remend)
		.bind(json_or_null(&prefs.theme_css_vars))
		.bind(json_or_null(&prefs.custom_theme_urls))
		.fetch_one(pool)
		.await
	}
}
