//! Shared application state.

use crate::utils::tools::McpConnectionPool;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};

/// Holds pending client-side tool calls keyed by call ID.
///
/// When the streaming loop needs the user's browser to execute a local MCP
/// tool call, it registers a oneshot channel here and then emits a
/// `ClientToolCall` SSE event. The client executes the tool, POSTs the result
/// to the submit endpoint, and `resolve` sends it through the channel so the
/// loop can continue.
#[derive(Clone, Default)]
pub struct ClientToolPending(Arc<RwLock<HashMap<String, oneshot::Sender<serde_json::Value>>>>);

impl std::fmt::Debug for ClientToolPending {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientToolPending").finish()
	}
}

impl ClientToolPending {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a pending call and return the receiver the streaming loop waits on.
	pub async fn register(&self, call_id: String) -> oneshot::Receiver<serde_json::Value> {
		let (tx, rx) = oneshot::channel();
		self.0.write().await.insert(call_id, tx);
		rx
	}

	/// Deliver a result from the client. Returns `true` if the call was found and delivered.
	pub async fn resolve(&self, call_id: &str, result: serde_json::Value) -> bool {
		if let Some(tx) = self.0.write().await.remove(call_id) {
			tx.send(result).is_ok()
		} else {
			false
		}
	}

	/// Remove a pending call without delivering a result (used on timeout).
	pub async fn cancel(&self, call_id: &str) {
		self.0.write().await.remove(call_id);
	}
}

/// Shared application state containing the database pool and shared caches.
#[derive(Clone, Debug)]
pub struct JobState {
	pub db: PgPool,
	/// Process-lifetime cache of initialized MCP clients keyed by `mcp_server_id`.
	pub mcp_pool: McpConnectionPool,
	/// Pending client-side MCP tool calls waiting for the browser to submit results.
	pub client_tool_pending: ClientToolPending,
}
