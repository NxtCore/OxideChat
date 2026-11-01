use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

mod mcp_scope;
mod mcp_server;
mod tool;
mod tool_execution;
mod tool_function;
mod wasm_blob;
mod requests;
mod responses;

pub use mcp_scope::McpScope;
pub use requests::*;
pub use responses::*;

pub type ToolDb = sqlx::PgPool;

#[derive(Debug, thiserror::Error)]
/// Failures produced while loading or validating tool settings.
pub enum ToolSettingsError {
	#[error("tool not found")]
	NotFound,
	#[error("invalid tool settings")]
	Invalid,
	#[error("database error: {0}")]
	Database(#[from] sqlx::Error),
	#[error("secret protection error: {0}")]
	Encryption(#[from] crate::utils::encryption::EncryptionError),
}

impl From<ToolSettingsError> for crate::types::AppError {
	fn from(error: ToolSettingsError) -> Self {
		use crate::utils::response::ErrorCode;
		match error {
			ToolSettingsError::NotFound => ErrorCode::NotFound.into(),
			ToolSettingsError::Invalid => ErrorCode::ValidationFailed.into(),
			ToolSettingsError::Database(error) => Self::Database(error),
			ToolSettingsError::Encryption(error) => {
				tracing::error!(%error, "failed to protect tool settings");
				ErrorCode::InternalError.into()
			}
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "tool_source_kind", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ToolSourceKind {
	Builtin,
	Wasm,
	Mcp,
	Http,
}

pub struct WasmBlob;

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

pub struct ToolExecution;

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

impl BaseType for Tool {}
impl BaseType for ToolFunction {}
impl BaseType for McpServer {}
