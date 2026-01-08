//! Tool types for OxideChat API.
//!
//! Types for managing custom tools, MCP servers, and tool executions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============= Enums =============

/// Tool source kind enum matching the database enum
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

// ============= Database Rows =============

/// WASM blob storage database row
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

/// Tool database row
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
	pub is_public: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// User-provided tool settings database row
#[derive(Debug, Clone, FromRow)]
pub struct UserToolSettings {
	pub id: Uuid,
	pub user_id: Uuid,
	pub tool_id: Uuid,
	pub settings: serde_json::Value,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// MCP Server database row
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

/// Tool execution audit log database row
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

// ============= Source Config Types =============

/// WASM source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSourceConfig {
	pub wasm_blob_id: Uuid,
	pub entrypoint: String,
	pub compiled_from: Option<String>,
}

/// MCP source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSourceConfig {
	pub mcp_server_id: Uuid,
	pub tool_name: String,
}

/// HTTP source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSourceConfig {
	pub url: String,
	pub method: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
	pub body_template: Option<String>,
}

/// Builtin source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinSourceConfig {
	pub builtin_id: String,
}

/// MCP stdio transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioConfig {
	pub command: String,
	#[serde(default)]
	pub args: Vec<String>,
	#[serde(default)]
	pub env: std::collections::HashMap<String, String>,
}

/// MCP SSE transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSseConfig {
	pub url: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
}

// ============= Request DTOs =============

/// Request to create a new tool
#[derive(Debug, Deserialize)]
pub struct CreateToolRequest {
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub source_config: serde_json::Value,
	pub input_schema: serde_json::Value,
	#[serde(default)]
	pub settings_schema: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
	#[serde(default)]
	pub is_public: bool,
}

fn default_true() -> bool {
	true
}

/// Request to update an existing tool
#[derive(Debug, Deserialize)]
pub struct UpdateToolRequest {
	pub name: Option<String>,
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_config: Option<serde_json::Value>,
	pub input_schema: Option<serde_json::Value>,
	pub settings_schema: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
	pub is_public: Option<bool>,
}

/// Request to upload WASM source or binary
#[derive(Debug, Deserialize)]
pub struct UploadWasmRequest {
	pub filename: String,
	/// Base64 encoded file content
	pub content: String,
	/// Force compilation even if file appears to be WASM
	#[serde(default)]
	pub force_compile: bool,
}

/// Request to create MCP server
#[derive(Debug, Deserialize)]
pub struct CreateMcpServerRequest {
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

/// Request to update MCP server
#[derive(Debug, Deserialize)]
pub struct UpdateMcpServerRequest {
	pub name: Option<String>,
	pub transport: Option<String>,
	pub connection_config: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

/// Request to set user tool settings
#[derive(Debug, Deserialize)]
pub struct SetToolSettingsRequest {
	pub settings: serde_json::Value,
}

/// Request to test tool execution
#[derive(Debug, Deserialize)]
pub struct TestToolRequest {
	pub input: serde_json::Value,
}

// ============= Response DTOs =============

/// Tool response
#[derive(Debug, Serialize)]
pub struct ToolResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub input_schema: serde_json::Value,
	pub settings_schema: serde_json::Value,
	pub is_enabled: bool,
	pub is_public: bool,
	pub has_user_settings: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl From<Tool> for ToolResponse {
	fn from(t: Tool) -> Self {
		Self {
			id: t.id,
			owner_id: t.owner_id,
			name: t.name,
			display_name: t.display_name,
			description: t.description,
			icon: t.icon,
			source_kind: t.source_kind,
			input_schema: t.input_schema,
			settings_schema: t.settings_schema,
			is_enabled: t.is_enabled,
			is_public: t.is_public,
			has_user_settings: false, // Set by query
			created_at: t.created_at,
			updated_at: t.updated_at,
		}
	}
}

/// MCP Server response
#[derive(Debug, Serialize)]
pub struct McpServerResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub transport: String,
	pub is_enabled: bool,
	pub last_health_check: Option<DateTime<Utc>>,
	pub health_status: Option<String>,
	pub discovered_tools: Vec<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl From<McpServer> for McpServerResponse {
	fn from(s: McpServer) -> Self {
		Self {
			id: s.id,
			owner_id: s.owner_id,
			name: s.name,
			transport: s.transport,
			is_enabled: s.is_enabled,
			last_health_check: s.last_health_check,
			health_status: s.health_status,
			discovered_tools: vec![], // Set by discovery
			created_at: s.created_at,
			updated_at: s.updated_at,
		}
	}
}

/// WASM upload response
#[derive(Debug, Serialize)]
pub struct UploadWasmResponse {
	pub blob_id: Uuid,
	pub size_bytes: i32,
	pub sha256_hash: String,
	pub compiled_from: Option<String>,
}

/// Tool test response
#[derive(Debug, Serialize)]
pub struct TestToolResponse {
	pub success: bool,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: i32,
}

/// MCP discovery response
#[derive(Debug, Serialize)]
pub struct McpDiscoveryResponse {
	pub tools: Vec<McpDiscoveredTool>,
	pub server_name: String,
	pub server_version: Option<String>,
}

/// Discovered MCP tool
#[derive(Debug, Serialize)]
pub struct McpDiscoveredTool {
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
}

/// Tool execution log response
#[derive(Debug, Serialize)]
pub struct ToolExecutionResponse {
	pub id: Uuid,
	pub tool_id: Option<Uuid>,
	pub tool_name: Option<String>,
	pub tool_call_id: String,
	pub input_args: serde_json::Value,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: Option<i32>,
	pub created_at: DateTime<Utc>,
}

// ============= Conversion to omniference ToolSpec =============

impl Tool {
	/// Convert to omniference ToolSpec for LLM requests
	pub fn to_tool_spec(&self) -> omniference::types::ToolSpec {
		omniference::types::ToolSpec::JsonSchema {
			name: self.name.clone(),
			description: self.description.clone(),
			schema: self.input_schema.clone(),
			strict: Some(false),
		}
	}
}
