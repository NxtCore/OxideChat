//! Admin (global/system) MCP server management routes.
//!
//! Admins manage `owner_id IS NULL` MCP servers available to all users. Both
//! remote (SSE / Streamable-HTTP) and server-side `stdio` transports are
//! supported; `stdio` is gated behind the `allow_server_stdio_mcp` instance flag
//! because it runs arbitrary commands on the `OxideChat` host.

use crate::types::consts::{ADMIN_TOOLS_EDIT, ADMIN_TOOLS_VIEW};
use crate::types::tools::*;
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/mcp-servers
pub async fn list_servers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;
	json_ok(McpScope::System.list(&state.db).await?)
}

/// POST /api/v1/admin/mcp-servers
pub async fn create_server(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<CreateMcpServerRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let response = McpScope::System.create(&state.db, &req).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// GET /api/v1/admin/mcp-servers/:id
pub async fn get_server(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;

	let server = McpServer::find_scoped(&state.db, &id, None).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(server.into_response_with_tools(&state.db, None).await)
}

/// PUT /api/v1/admin/mcp-servers/:id
pub async fn update_server(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateMcpServerRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let response = McpScope::System.update(&state.db, &id, &req).await?;
	state.mcp_pool.evict(&id).await;
	json_ok(response)
}

/// DELETE /api/v1/admin/mcp-servers/:id
pub async fn delete_server(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	if !McpServer::delete_scoped(&state.db, &id, None).await? {
		return Err(ErrorCode::NotFound.into());
	}
	state.mcp_pool.evict(&id).await;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// POST /api/v1/admin/mcp-servers/:id/discover
pub async fn discover_server(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let server = McpServer::find_scoped(&state.db, &id, None).await?.ok_or(ErrorCode::NotFound)?;
	server.run_discovery(&state.db, &state.mcp_pool, None).await
}

/// POST /api/v1/admin/mcp-servers/:id/health-check
pub async fn health_check(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;

	let server = McpServer::find_scoped(&state.db, &id, None).await?.ok_or(ErrorCode::NotFound)?;
	state.mcp_pool.evict(&server.id).await;
	let status = if server.discover().await.is_ok() { "healthy" } else { "unhealthy" };
	let _ = McpServer::set_health(&state.db, &server.id, status).await;

	json_ok(serde_json::json!({ "health_status": status }))
}
