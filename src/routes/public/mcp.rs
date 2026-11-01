//! User-owned MCP server management routes.
//!
//! Any authenticated user may register, discover, and remove their own remote
//! (Streamable HTTP) MCP servers. Server-side `stdio` transports are rejected
//! for user-owned servers — those are reserved for admins.

use crate::types::tools::*;
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use crate::utils::tools::mcp::McpToolInfo;
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/mcp-servers
pub async fn list_servers(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(McpScope::User(user.id).list(&state.db).await?)
}

/// POST /api/v1/mcp-servers
pub async fn create_server(State(state): State<Arc<JobState>>, user: AuthUser, Json(req): Json<CreateMcpServerRequest>) -> Result<Response, AppError> {
	let response = McpScope::User(user.id).create(&state.db, &req).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// GET /api/v1/mcp-servers/:id
pub async fn get_server(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let server = McpServer::find_scoped(&state.db, &id, Some(&user.id)).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(server.into_response_with_tools(&state.db, Some(&user.id)).await)
}

/// PUT /api/v1/mcp-servers/:id
pub async fn update_server(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateMcpServerRequest>) -> Result<Response, AppError> {
	let response = McpScope::User(user.id).update(&state.db, &id, &req).await?;
	state.mcp_pool.evict(&id).await;
	json_ok(response)
}

/// DELETE /api/v1/mcp-servers/:id
pub async fn delete_server(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	if !McpServer::delete_scoped(&state.db, &id, Some(&user.id)).await? {
		return Err(ErrorCode::NotFound.into());
	}
	state.mcp_pool.evict(&id).await;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// POST /api/v1/mcp-servers/:id/sync-tools
///
/// Accepts tool schemas discovered client-side (browser calling the local MCP)
/// and persists them as `Tool` records, exactly like server-side discover would.
/// Used for user-owned MCP servers whose URL is only reachable from the user's
/// machine (e.g. localhost).
pub async fn sync_tools_from_client(
	State(state): State<Arc<JobState>>,
	user: AuthUser,
	Path(id): Path<Uuid>,
	Json(req): Json<SyncMcpToolsRequest>,
) -> Result<Response, AppError> {
	let server = McpServer::find_scoped(&state.db, &id, Some(&user.id)).await?.ok_or(ErrorCode::NotFound)?;

	let discovered: Vec<McpToolInfo> = req
		.tools
		.into_iter()
		.map(|t| McpToolInfo {
			name: t.name,
			description: t.description,
			input_schema: t.input_schema,
		})
		.collect();

	server.sync_tools(&state.db, Some(&user.id), &discovered).await?;

	let _ = McpServer::set_health(&state.db, &server.id, "healthy").await;
	state.mcp_pool.evict(&server.id).await;

	json_ok(McpDiscoveryResponse::new(server.name, discovered))
}

/// POST /api/v1/mcp-servers/:id/discover
///
/// Connect to the server, list its tools, and (re)generate the user's `Tool`
/// records for it so they become available in the chat tool selector.
pub async fn discover_server(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let server = McpServer::find_scoped(&state.db, &id, Some(&user.id)).await?.ok_or(ErrorCode::NotFound)?;
	server.run_discovery(&state.db, &state.mcp_pool, Some(&user.id)).await
}
