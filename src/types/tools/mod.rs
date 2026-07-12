use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

mod repository;
mod requests;
mod responses;

pub use requests::*;
pub use responses::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "tool_source_kind", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolSourceKind {
	Builtin,
	Wasm,
	Mcp,
	Http,
}

impl ToolSourceKind {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Builtin => "BUILTIN",
			Self::Wasm => "WASM",
			Self::Mcp => "MCP",
			Self::Http => "HTTP",
		}
	}

	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"BUILTIN" => Some(Self::Builtin),
			"WASM" => Some(Self::Wasm),
			"MCP" => Some(Self::Mcp),
			"HTTP" => Some(Self::Http),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, FromRow)]
pub struct WasmBlob {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub original_filename: Option<String>,
	pub compiled_from: Option<String>,
	pub blob: Vec<u8>,
	pub size_bytes: i32,
	pub sha256_hash: String,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Tool {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub source_config: serde_json::Value,
	pub input_schema: serde_json::Value,
	pub settings_schema: serde_json::Value,
	pub is_enabled: bool,
	pub system_prompt: Option<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ToolFunction {
	pub id: Uuid,
	pub tool_id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
	pub sort_order: i32,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserToolSettings {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub tool_id: Uuid,
	pub settings: serde_json::Value,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpServer {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
	pub is_enabled: bool,
	pub last_health_check: Option<DateTime<Utc>>,
	pub health_status: Option<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ToolExecution {
	pub id: Uuid,
	pub message_id: Option<Uuid>,
	pub tool_id: Option<Uuid>,
	pub tool_call_id: String,
	pub input_args: serde_json::Value,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: Option<i32>,
	pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSourceConfig {
	pub wasm_blob_id: Uuid,
	pub entrypoint: String,
	pub compiled_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSourceConfig {
	pub mcp_server_id: Uuid,
	pub tool_name: String,
}

impl McpSourceConfig {
	#[must_use]
	pub fn from_tool(tool: &Tool) -> Option<Self> {
		if tool.source_kind != ToolSourceKind::Mcp {
			return None;
		}
		serde_json::from_value(tool.source_config.clone()).ok()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSourceConfig {
	pub url: String,
	pub method: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
	pub body_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinSourceConfig {
	pub builtin_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioConfig {
	pub command: String,
	#[serde(default)]
	pub args: Vec<String>,
	#[serde(default)]
	pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHttpConfig {
	pub url: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
}

impl BaseType for WasmBlob {}
impl BaseType for Tool {}
impl BaseType for ToolFunction {}
impl BaseType for UserToolSettings {}
impl BaseType for McpServer {}
impl BaseType for ToolExecution {}
