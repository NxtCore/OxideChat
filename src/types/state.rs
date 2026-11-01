//! Shared application state.

use crate::types::{BudgetLedger, GatewayLimiter};
use crate::utils::login_limit::LoginRateLimiter;
use crate::utils::tools::McpConnectionPool;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};
use uuid::Uuid;

type PendingSenders = HashMap<(Uuid, String), oneshot::Sender<serde_json::Value>>;

/// Per-request context populated by the request middleware.
///
/// Additional request-derived values can be added here without changing every
/// handler's extractor type.
#[derive(Clone, Debug)]
pub struct RequestContext {
	pub user: Option<crate::types::User>,
}

/// A registered client tool call; dropping it before the result arrives removes the registration.
pub struct PendingCall {
	rx: oneshot::Receiver<serde_json::Value>,
	pending: ClientToolPending,
	call_id: String,
	user_id: Uuid,
}

impl PendingCall {
	/// Waits for the client result, returning `None` when the timeout passes or the call is cancelled.
	pub async fn wait(&mut self, timeout: std::time::Duration) -> Option<serde_json::Value> {
		tokio::time::timeout(timeout, &mut self.rx).await.ok()?.ok()
	}
}

impl Drop for PendingCall {
	fn drop(&mut self) {
		if !matches!(self.rx.try_recv(), Err(oneshot::error::TryRecvError::Empty)) {
			return;
		}
		let key = (self.user_id, std::mem::take(&mut self.call_id));
		if let Ok(mut map) = self.pending.0.try_write() {
			map.remove(&key);
		} else if let Ok(runtime) = tokio::runtime::Handle::try_current() {
			let pending = self.pending.clone();
			runtime.spawn(async move {
				pending.0.write().await.remove(&key);
			});
		}
	}
}

/// Holds pending client-side tool calls keyed by call ID.
///
/// When the streaming loop needs the user's browser to execute a local MCP
/// tool call, it registers a oneshot channel here before it emits a
/// `ClientToolCall` SSE event. The client executes the tool, POSTs the result
/// to the submit endpoint, and `resolve` sends it through the channel so the
/// loop can continue.
#[derive(Clone, Default)]
pub struct ClientToolPending(Arc<RwLock<PendingSenders>>);

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

	/// Register a pending call and return the handle the streaming loop waits on.
	///
	/// Dropping the handle before a result arrives removes the registration.
	pub async fn register(&self, call_id: String, user_id: Uuid) -> PendingCall {
		let (tx, rx) = oneshot::channel();
		self.0.write().await.insert((user_id, call_id.clone()), tx);
		PendingCall {
			rx,
			pending: self.clone(),
			call_id,
			user_id,
		}
	}

	/// Deliver a result from the client. Returns `true` if the call was found and delivered.
	pub async fn resolve(&self, call_id: &str, user_id: Uuid, result: serde_json::Value) -> bool {
		if let Some(tx) = self.0.write().await.remove(&(user_id, call_id.to_string())) {
			tx.send(result).is_ok()
		} else {
			false
		}
	}
}

/// Shared application state containing the database pool and shared caches.
#[derive(Clone, Debug)]
pub struct JobState {
	pub gateway_limiter: GatewayLimiter,
	pub budget_ledger: BudgetLedger,
	pub login_limiter: LoginRateLimiter,
	pub db: PgPool,
	/// Process-lifetime cache of initialized MCP clients keyed by `mcp_server_id`.
	pub mcp_pool: McpConnectionPool,
	/// Pending client-side MCP tool calls waiting for the browser to submit results.
	pub client_tool_pending: ClientToolPending,
}
