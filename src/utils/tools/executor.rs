use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ToolError {
	#[error("Tool not found: {0}")]
	NotFound(String),

	#[error("Tool execution failed: {0}")]
	ExecutionFailed(String),

	#[error("Tool timeout after {0}ms")]
	Timeout(u64),

	#[error("Invalid input: {0}")]
	InvalidInput(String),

	#[error("Missing required setting: {0}")]
	MissingSetting(String),

	#[error("WASM error: {0}")]
	WasmError(String),

	#[error("HTTP error: {0}")]
	HttpError(String),

	#[error("MCP error: {0}")]
	McpError(String),

	#[error("Compilation error: {0}")]
	CompilationError(String),

	#[error("Internal error: {0}")]
	Internal(String),
}

#[derive(Debug, Clone)]
pub struct ToolContext {
	pub user_id: Option<Uuid>,
	pub settings: Value,
	pub timeout_ms: Option<u64>,
	pub function_name: Option<String>,
}

impl Default for ToolContext {
	fn default() -> Self {
		Self {
			user_id: None,
			settings: Value::Object(serde_json::Map::new()),
			timeout_ms: Some(30000),
			function_name: None,
		}
	}
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
	/// Execute the tool with the given input
	///
	/// # Arguments
	/// * `input` - JSON input matching the tool's input_schema
	/// * `ctx` - Execution context with user settings
	///
	/// # Returns
	/// JSON output from the tool, or an error
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError>;

	/// Get the tool name for logging/debugging
	fn name(&self) -> &str;
}
