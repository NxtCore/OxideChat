use super::ToolSourceKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateToolFunctionRequest {
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateToolRequest {
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub source_config: serde_json::Value,
	#[serde(default)]
	pub input_schema: Option<serde_json::Value>,
	#[serde(default)]
	pub functions: Vec<CreateToolFunctionRequest>,
	#[serde(default)]
	pub settings_schema: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

pub(super) fn default_true() -> bool {
	true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateToolFunctionRequest {
	pub id: Option<Uuid>,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateToolRequest {
	pub name: Option<String>,
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_config: Option<serde_json::Value>,
	pub input_schema: Option<serde_json::Value>,
	pub functions: Option<Vec<UpdateToolFunctionRequest>>,
	pub delete_function_ids: Option<Vec<Uuid>>,
	pub settings_schema: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UploadWasmRequest {
	pub filename: String,
	pub content: String,
	#[serde(default)]
	pub force_compile: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateMcpServerRequest {
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMcpServerRequest {
	pub name: Option<String>,
	pub transport: Option<String>,
	pub connection_config: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ClientDiscoveredTool {
	pub name: String,
	pub description: Option<String>,
	#[serde(alias = "inputSchema")]
	pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SyncMcpToolsRequest {
	pub tools: Vec<ClientDiscoveredTool>,
}

#[derive(Debug, Deserialize)]
pub struct SetToolSettingsRequest {
	pub settings: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct TestToolRequest {
	pub input: serde_json::Value,
	#[serde(default)]
	pub function_name: Option<String>,
}
