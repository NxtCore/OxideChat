//! MCP (Model Context Protocol) client for external tool servers.
//!
//! Supports the stdio transport and the Streamable HTTP transport (MCP
//! 2025-03-26+) for connecting to MCP servers.

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

#[async_trait]
trait McpTransport: Send + Sync {
	async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, ToolError>;
	async fn close(&self) -> Result<(), ToolError>;
}

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

		let mut id = self.request_id.lock().await;
		request.id = *id;
		*id += 1;
		drop(id);

		let stdin = child.stdin.as_mut().ok_or_else(|| ToolError::McpError("No stdin available".to_string()))?;

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

/// MCP protocol version advertised on initialize and on every subsequent request.
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// Streamable HTTP transport (MCP 2025-03-26+).
///
/// A single endpoint receives JSON-RPC via `POST`. The server may answer with a
/// plain JSON body or a `text/event-stream` (SSE) response carrying the JSON-RPC
/// message(s); both are handled. The `Mcp-Session-Id` returned on `initialize`
/// is echoed back on every following request.
struct StreamableHttpTransport {
	url: String,
	headers: HashMap<String, String>,
	client: reqwest::Client,
	request_id: Arc<Mutex<u64>>,
	session_id: Arc<Mutex<Option<String>>>,
}

impl StreamableHttpTransport {
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
			session_id: Arc::new(Mutex::new(None)),
		})
	}
}

/// A JSON-RPC response with no payload, used for accepted notifications.
fn empty_response(id: u64) -> JsonRpcResponse {
	JsonRpcResponse {
		jsonrpc: "2.0".to_string(),
		id,
		result: None,
		error: None,
	}
}

/// Extract the JSON-RPC response for `expected_id` from an SSE (`text/event-stream`)
/// body. Non-matching data events (e.g. server notifications) are skipped; the
/// first parseable message is used as a fallback if no id matches.
fn parse_sse_response(body: &str, expected_id: u64) -> Result<JsonRpcResponse, ToolError> {
	let mut data_lines: Vec<String> = Vec::new();
	let mut events: Vec<String> = Vec::new();

	for line in body.lines() {
		if line.is_empty() {
			if !data_lines.is_empty() {
				events.push(data_lines.join("\n"));
				data_lines.clear();
			}
		} else if let Some(rest) = line.strip_prefix("data:") {
			data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
		}
	}
	if !data_lines.is_empty() {
		events.push(data_lines.join("\n"));
	}

	let mut fallback: Option<JsonRpcResponse> = None;
	for payload in events {
		if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&payload) {
			if resp.id == expected_id {
				return Ok(resp);
			}
			if fallback.is_none() {
				fallback = Some(resp);
			}
		}
	}

	fallback.ok_or_else(|| ToolError::McpError("No JSON-RPC response found in SSE stream".to_string()))
}

#[async_trait]
impl McpTransport for StreamableHttpTransport {
	async fn send(&self, mut request: JsonRpcRequest) -> Result<JsonRpcResponse, ToolError> {
		let expected_id = {
			let mut id = self.request_id.lock().await;
			request.id = *id;
			*id += 1;
			*id - 1
		};

		let mut req = self.client.post(&self.url);
		for (key, value) in &self.headers {
			req = req.header(key.as_str(), value.as_str());
		}
		req = req
			.header(reqwest::header::CONTENT_TYPE, "application/json")
			.header(reqwest::header::ACCEPT, "application/json, text/event-stream")
			.header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION);

		if let Some(session_id) = self.session_id.lock().await.clone() {
			req = req.header("Mcp-Session-Id", session_id);
		}

		let response = req
			.json(&request)
			.send()
			.await
			.map_err(|e| ToolError::McpError(format!("HTTP request failed: {e}")))?;

		if let Some(session_id) = response.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
			*self.session_id.lock().await = Some(session_id.to_string());
		}

		let status = response.status();
		let content_type = response
			.headers()
			.get(reqwest::header::CONTENT_TYPE)
			.and_then(|v| v.to_str().ok())
			.unwrap_or("")
			.to_lowercase();

		if !status.is_success() {
			let body = response.text().await.unwrap_or_default();
			return Err(ToolError::McpError(format!("HTTP {status}: {body}")));
		}

		if status == reqwest::StatusCode::ACCEPTED {
			return Ok(empty_response(expected_id));
		}

		let body = response.text().await.map_err(|e| ToolError::McpError(format!("Failed to read response: {e}")))?;

		if content_type.contains("text/event-stream") {
			return parse_sse_response(&body, expected_id);
		}

		if body.trim().is_empty() {
			return Ok(empty_response(expected_id));
		}

		serde_json::from_str(&body).map_err(|e| ToolError::McpError(format!("Failed to parse response: {e}")))
	}

	async fn close(&self) -> Result<(), ToolError> {
		if let Some(session_id) = self.session_id.lock().await.clone() {
			let _ = self.client.delete(&self.url).header("Mcp-Session-Id", session_id).send().await;
		}
		Ok(())
	}
}

pub struct McpClient {
	transport: Box<dyn McpTransport>,
	server_name: String,
}

impl McpClient {
	/// Create a new MCP client with stdio transport
	pub async fn new_stdio(server_name: String, command: &str, args: &[String], env: &HashMap<String, String>) -> Result<Self, ToolError> {
		let transport = StdioTransport::new(command, args, env).await?;
		let client = Self {
			transport: Box::new(transport),
			server_name,
		};
		client.initialize().await?;
		Ok(client)
	}

	/// Create a new MCP client with the Streamable HTTP transport.
	///
	/// The connection is initialized before returning so the client is ready to use.
	pub async fn new_http(server_name: String, url: String, headers: HashMap<String, String>) -> Result<Self, ToolError> {
		let transport = StreamableHttpTransport::new(url, headers)?;
		let client = Self {
			transport: Box::new(transport),
			server_name,
		};
		client.initialize().await?;
		Ok(client)
	}

	/// Initialize the MCP connection
	pub async fn initialize(&self) -> Result<(), ToolError> {
		let request = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "initialize".to_string(),
			params: Some(json!({
				"protocolVersion": MCP_PROTOCOL_VERSION,
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

		let notification = JsonRpcRequest {
			jsonrpc: "2.0".to_string(),
			id: 0,
			method: "notifications/initialized".to_string(),
			params: None,
		};
		let _ = self.transport.send(notification).await;

		Ok(())
	}

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

		let text: String = result.content.iter().filter_map(|c| c.text.as_deref()).collect::<Vec<_>>().join("\n");

		Ok(json!({ "result": text }))
	}

	pub async fn close(&self) -> Result<(), ToolError> {
		self.transport.close().await
	}
}

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

/// A pooled MCP client together with the time it was last used.
struct PooledClient {
	client: Arc<McpClient>,
	last_used: Mutex<std::time::Instant>,
}

/// Process-lifetime cache of initialized MCP clients keyed by `mcp_server_id`.
///
/// Reusing a client lets a single chat's multiple tool calls share one
/// initialized connection instead of spawning/killing a server per call.
#[derive(Clone, Default)]
pub struct McpConnectionPool {
	inner: Arc<tokio::sync::RwLock<HashMap<uuid::Uuid, Arc<PooledClient>>>>,
}

impl std::fmt::Debug for McpConnectionPool {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("McpConnectionPool").finish()
	}
}

impl McpConnectionPool {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Return a cached client for the server, updating its last-used time.
	pub async fn get(&self, id: &uuid::Uuid) -> Option<Arc<McpClient>> {
		let map = self.inner.read().await;
		if let Some(pooled) = map.get(id) {
			*pooled.last_used.lock().await = std::time::Instant::now();
			Some(Arc::clone(&pooled.client))
		} else {
			None
		}
	}

	/// Insert a freshly connected client into the pool.
	pub async fn insert(&self, id: uuid::Uuid, client: Arc<McpClient>) {
		let pooled = Arc::new(PooledClient {
			client,
			last_used: Mutex::new(std::time::Instant::now()),
		});
		self.inner.write().await.insert(id, pooled);
	}

	/// Evict and close a client (e.g. on server update/delete or transport error).
	pub async fn evict(&self, id: &uuid::Uuid) {
		let removed = self.inner.write().await.remove(id);
		if let Some(pooled) = removed {
			let _ = pooled.client.close().await;
		}
	}

	/// Close and remove clients idle for longer than `max_idle`.
	///
	/// Returns the number of clients reaped.
	pub async fn reap_idle(&self, max_idle: std::time::Duration) -> usize {
		let mut expired = Vec::new();
		{
			let map = self.inner.read().await;
			for (id, pooled) in map.iter() {
				if pooled.last_used.lock().await.elapsed() > max_idle {
					expired.push(*id);
				}
			}
		}
		let mut closed = 0;
		let mut map = self.inner.write().await;
		for id in expired {
			if let Some(pooled) = map.remove(&id) {
				let _ = pooled.client.close().await;
				closed += 1;
			}
		}
		closed
	}
}
