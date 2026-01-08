//! MCP (Model Context Protocol) client for external tool servers.
//!
//! Supports both stdio and SSE transports for connecting to MCP servers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::executor::{ToolContext, ToolError, ToolExecutor};

// ============= MCP Protocol Types =============

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
	jsonrpc: String,
	id: u64,
	method: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
	jsonrpc: String,
	id: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	result: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
	code: i32,
	message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolInfo {
	pub name: String,
	pub description: Option<String>,
	#[serde(rename = "inputSchema")]
	pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListToolsResult {
	tools: Vec<McpToolInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CallToolResult {
	content: Vec<McpContent>,
	#[serde(rename = "isError", default)]
	is_error: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpContent {
	#[serde(rename = "type")]
	content_type: String,
	text: Option<String>,
}

// ============= MCP Transport Trait =============

#[async_trait]
trait McpTransport: Send + Sync {
	async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, ToolError>;
	async fn close(&self) -> Result<(), ToolError>;
}

// ============= Stdio Transport =============

struct StdioTransport {
	child: Arc<Mutex<Child>>,
	request_id: Arc<Mutex<u64>>,
}

impl StdioTransport {
	pub async fn new(command: &str, args: &[String], env: &HashMap<String, String>) -> Result<Self, ToolError> {
		let mut cmd = Command::new(command);
		cmd.args(args).envs(env.iter()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

		let child = cmd.spawn().map_err(|e| ToolError::McpError(format!("Failed to spawn MCP server: {e}")))?;

		Ok(Self {
			child: Arc::new(Mutex::new(child)),
			request_id: Arc::new(Mutex::new(1)),
		})
	}
}

#[async_trait]
impl McpTransport for StdioTransport {
	async fn send(&self, mut request: JsonRpcRequest) -> Result<JsonRpcResponse, ToolError> {
		let mut child = self.child.lock().await;

		// Get next request ID
		let mut id = self.request_id.lock().await;
		request.id = *id;
		*id += 1;
		drop(id);

		let stdin = child.stdin.as_mut().ok_or_else(|| ToolError::McpError("No stdin available".to_string()))?;

		// Send request
		let request_str = serde_json::to_string(&request).map_err(|e| ToolError::McpError(format!("Failed to serialize request: {e}")))?;

		stdin
			.write_all(request_str.as_bytes())
			.await
			.map_err(|e| ToolError::McpError(format!("Failed to write to stdin: {e}")))?;
		stdin
			.write_all(b"\n")
			.await
			.map_err(|e| ToolError::McpError(format!("Failed to write newline: {e}")))?;
		stdin.flush().await.map_err(|e| ToolError::McpError(format!("Failed to flush stdin: {e}")))?;

		// Read response
		let stdout = child.stdout.as_mut().ok_or_else(|| ToolError::McpError("No stdout available".to_string()))?;
		let mut reader = BufReader::new(stdout);
		let mut response_line = String::new();
		reader
			.read_line(&mut response_line)
			.await
			.map_err(|e| ToolError::McpError(format!("Failed to read response: {e}")))?;

		let response: JsonRpcResponse = serde_json::from_str(&response_line).map_err(|e| ToolError::McpError(format!("Failed to parse response: {e}")))?;

		Ok(response)
	}

	async fn close(&self) -> Result<(), ToolError> {
		let mut child = self.child.lock().await;
		child.kill().await.map_err(|e| ToolError::McpError(format!("Failed to kill MCP server: {e}")))?;
		Ok(())
	}
}

// ============= SSE Transport =============

struct SseTransport {
	url: String,
	headers: HashMap<String, String>,
	client: reqwest::Client,
	request_id: Arc<Mutex<u64>>,
}

impl SseTransport {
	pub fn new(url: String, headers: HashMap<String, String>) -> Result<Self, ToolError> {
		let client = reqwest::Client::builder()
			.timeout(std::time::Duration::from_secs(30))
			.build()
			.map_err(|e| ToolError::McpError(format!("Failed to create HTTP client: {e}")))?;

		Ok(Self {
			url,
			headers,
			client,
			request_id: Arc::new(Mutex::new(1)),
		})
	}
}

#[async_trait]
impl McpTransport for SseTransport {
	async fn send(&self, mut request: JsonRpcRequest) -> Result<JsonRpcResponse, ToolError> {
		// Get next request ID
		let mut id = self.request_id.lock().await;
		request.id = *id;
		*id += 1;
		drop(id);

		let mut req = self.client.post(&self.url);
		for (key, value) in &self.headers {
			req = req.header(key.as_str(), value.as_str());
		}
		req = req.header("Content-Type", "application/json");

		let response = req
			.json(&request)
			.send()
			.await
			.map_err(|e| ToolError::McpError(format!("HTTP request failed: {e}")))?;

		if !response.status().is_success() {
			let status = response.status();
			let body = response.text().await.unwrap_or_default();
			return Err(ToolError::McpError(format!("HTTP {status}: {body}")));
		}

		let json_response: JsonRpcResponse = response.json().await.map_err(|e| ToolError::McpError(format!("Failed to parse response: {e}")))?;

		Ok(json_response)
	}

	async fn close(&self) -> Result<(), ToolError> {
		// SSE transport doesn't need cleanup
		Ok(())
	}
}

// ============= MCP Client =============

pub struct McpClient {
	transport: Box<dyn McpTransport>,
	server_name: String,
}

impl McpClient {
	/// Create a new MCP client with stdio transport
	pub async fn new_stdio(server_name: String, command: &str, args: &[String], env: &HashMap<String, String>) -> Result<Self, ToolError> {
		let transport = StdioTransport::new(command, args, env).await?;
		let mut client = Self {
			transport: Box::new(transport),
			server_name,
		};
		client.initialize().await?;
		Ok(client)
	}

	/// Create a new MCP client with SSE transport
	pub fn new_sse(server_name: String, url: String, headers: HashMap<String, String>) -> Result<Self, ToolError> {
		let transport = SseTransport::new(url, headers)?;
		Ok(Self {
			transport: Box::new(transport),
			server_name,
		})
	}

	/// Initialize the MCP connection
	async fn initialize(&mut self) -> Result<(), ToolError> {
		let request = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "initialize".to_string(),
			params: Some(json!({
				"protocolVersion": "2024-11-05",
				"capabilities": {},
				"clientInfo": {
					"name": "OxideChat",
					"version": "0.1.0"
				}
			})),
		};

		let response = self.transport.send(request).await?;

		if let Some(error) = response.error {
			return Err(ToolError::McpError(format!("Initialize failed: {}", error.message)));
		}

		// Send initialized notification
		let notification = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "notifications/initialized".to_string(),
			params: None,
		};
		let _ = self.transport.send(notification).await;

		Ok(())
	}

	/// List available tools from the MCP server
	pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>, ToolError> {
		let request = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "tools/list".to_string(),
			params: None,
		};

		let response = self.transport.send(request).await?;

		if let Some(error) = response.error {
			return Err(ToolError::McpError(format!("List tools failed: {}", error.message)));
		}

		let result: ListToolsResult =
			serde_json::from_value(response.result.unwrap_or_default()).map_err(|e| ToolError::McpError(format!("Failed to parse tools list: {e}")))?;

		Ok(result.tools)
	}

	/// Call a tool on the MCP server
	pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, ToolError> {
		let request = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "tools/call".to_string(),
			params: Some(json!({
				"name": tool_name,
				"arguments": arguments
			})),
		};

		let response = self.transport.send(request).await?;

		if let Some(error) = response.error {
			return Err(ToolError::McpError(format!("Tool call failed: {}", error.message)));
		}

		let result: CallToolResult =
			serde_json::from_value(response.result.unwrap_or_default()).map_err(|e| ToolError::McpError(format!("Failed to parse tool result: {e}")))?;

		if result.is_error {
			let error_text: String = result.content.iter().filter_map(|c| c.text.as_deref()).collect::<Vec<_>>().join("\n");
			return Err(ToolError::ExecutionFailed(error_text));
		}

		// Combine all text content
		let text: String = result.content.iter().filter_map(|c| c.text.as_deref()).collect::<Vec<_>>().join("\n");

		Ok(json!({ "result": text }))
	}

	/// Close the MCP connection
	pub async fn close(&self) -> Result<(), ToolError> {
		self.transport.close().await
	}
}

// ============= MCP Tool Executor =============

/// Executor that wraps an MCP client for a specific tool
pub struct McpExecutor {
	client: Arc<McpClient>,
	tool_name: String,
}

impl McpExecutor {
	pub fn new(client: Arc<McpClient>, tool_name: String) -> Self {
		Self { client, tool_name }
	}
}

#[async_trait]
impl ToolExecutor for McpExecutor {
	async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
		self.client.call_tool(&self.tool_name, input).await
	}

	fn name(&self) -> &str {
		&self.tool_name
	}
}
