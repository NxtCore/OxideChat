use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

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

impl crate::types::BaseType for Workspace {}
impl crate::types::BaseType for Chat {}
impl crate::types::BaseType for Message {}
impl crate::types::BaseType for UserPreferences {}
