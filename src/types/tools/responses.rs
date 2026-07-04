use super::{McpServer, Tool, ToolFunction, ToolSourceKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
	pub sort_order: i32,
}

impl From<ToolFunction> for ToolFunctionResponse {
	fn from(f: ToolFunction) -> Self {
		Self {
			id: f.id,
			name: f.name,
			description: f.description,
			input_schema: f.input_schema,
			entrypoint: f.entrypoint,
			sort_order: f.sort_order,
		}
	}
}

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
	pub functions: Vec<ToolFunctionResponse>,
	pub settings_schema: serde_json::Value,
	pub is_enabled: bool,
	pub has_user_settings: bool,
	pub mcp_server_id: Option<Uuid>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl ToolResponse {
	pub fn from_tool_with_functions(t: Tool, functions: Vec<ToolFunction>) -> Self {
		let mcp_server_id = super::McpSourceConfig::from_tool(&t).map(|c| c.mcp_server_id);
		Self {
			id: t.id,
			owner_id: t.owner_id,
			name: t.name,
			display_name: t.display_name,
			description: t.description,
			icon: t.icon,
			source_kind: t.source_kind,
			input_schema: t.input_schema,
			functions: functions.into_iter().map(ToolFunctionResponse::from).collect(),
			settings_schema: t.settings_schema,
			is_enabled: t.is_enabled,
			has_user_settings: false,
			mcp_server_id,
			created_at: t.created_at,
			updated_at: t.updated_at,
		}
	}
}

impl From<Tool> for ToolResponse {
	fn from(t: Tool) -> Self {
		let mcp_server_id = super::McpSourceConfig::from_tool(&t).map(|c| c.mcp_server_id);
		Self {
			id: t.id,
			owner_id: t.owner_id,
			name: t.name,
			display_name: t.display_name,
			description: t.description,
			icon: t.icon,
			source_kind: t.source_kind,
			input_schema: t.input_schema,
			functions: vec![],
			settings_schema: t.settings_schema,
			is_enabled: t.is_enabled,
			has_user_settings: false,
			mcp_server_id,
			created_at: t.created_at,
			updated_at: t.updated_at,
		}
	}
}

#[derive(Debug, Serialize)]
pub struct McpServerResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
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
			connection_config: s.connection_config,
			is_enabled: s.is_enabled,
			last_health_check: s.last_health_check,
			health_status: s.health_status,
			discovered_tools: vec![],
			created_at: s.created_at,
			updated_at: s.updated_at,
		}
	}
}

impl McpServerResponse {
	pub fn from_server(s: McpServer, include_secrets: bool) -> Self {
		let mut response = Self::from(s);
		if !include_secrets {
			response.connection_config = mask_connection_config(response.connection_config);
		}
		response
	}
}

fn mask_connection_config(mut config: serde_json::Value) -> serde_json::Value {
	if let Some(headers) = config.get_mut("headers").and_then(serde_json::Value::as_object_mut) {
		for value in headers.values_mut() {
			*value = serde_json::Value::String("***".to_string());
		}
	}
	if let Some(env) = config.get_mut("env").and_then(serde_json::Value::as_object_mut) {
		for value in env.values_mut() {
			*value = serde_json::Value::String("***".to_string());
		}
	}
	config
}

#[derive(Debug, Serialize)]
pub struct UploadWasmResponse {
	pub blob_id: Uuid,
	pub size_bytes: i32,
	pub sha256_hash: String,
	pub compiled_from: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestToolResponse {
	pub success: bool,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: i32,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
	pub tool_id: Uuid,
	pub function_id: Option<Uuid>,
	pub output: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpDiscoveryResponse {
	pub tools: Vec<McpDiscoveredTool>,
	pub server_name: String,
	pub server_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpDiscoveredTool {
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionLogResponse {
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
