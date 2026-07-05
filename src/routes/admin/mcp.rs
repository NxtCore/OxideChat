//! Admin (global/system) MCP server management routes.
//!
//! Admins manage `owner_id IS NULL` MCP servers available to all users. Both
//! remote (SSE / Streamable-HTTP) and server-side `stdio` transports are
//! supported; `stdio` is gated behind the `allow_server_stdio_mcp` instance flag
//! because it runs arbitrary commands on the OxideChat host.

use crate::config::Config;
use crate::routes::public::auth::get_current_user;
use crate::routes::public::mcp::{normalize_remote_transport, run_discovery, validate_admin_remote_config};
use crate::types::JobState;
use crate::types::consts::{ADMIN_TOOLS_EDIT, ADMIN_TOOLS_VIEW};
use crate::types::tools::*;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

/// Normalize an admin-supplied transport, allowing `stdio` only when enabled.
fn normalize_admin_transport(transport: &str, connection_config: &serde_json::Value) -> Result<&'static str, ErrorCode> {
	if transport.eq_ignore_ascii_case("stdio") {
		if !Config::get().allow_server_stdio_mcp() {
			return Err(ErrorCode::Forbidden);
		}
		let ok = serde_json::from_value::<McpStdioConfig>(connection_config.clone())
			.map(|c| !c.command.trim().is_empty())
			.unwrap_or(false);
		return if ok { Ok("stdio") } else { Err(ErrorCode::ValidationFailed) };
	}

	match normalize_remote_transport(transport) {
		Some(norm) if validate_admin_remote_config(connection_config) => Ok(norm),
		_ => Err(ErrorCode::ValidationFailed),
	}
}

/// GET /api/v1/admin/mcp-servers
pub async fn list_servers(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let servers = match McpServer::list_system(&state.db).await {
		Ok(s) => s,
		Err(e) => {
			eprintln!("[MCP] Failed to list system servers: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut responses = Vec::with_capacity(servers.len());
	for server in servers {
		responses.push(server.to_response_with_tools(&state.db, None).await);
	}
	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}

/// POST /api/v1/admin/mcp-servers
pub async fn create_server(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateMcpServerRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if req.name.trim().is_empty() {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}
	let transport = match normalize_admin_transport(&req.transport, &req.connection_config) {
		Ok(t) => t,
		Err(code) => return ErrorBuilder::new(code).build(),
	};

	match McpServer::create(&state.db, None, req.name.trim(), transport, &req.connection_config, req.is_enabled).await {
		Ok(server) => {
			let response = server.to_response_with_tools(&state.db, None).await;
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[MCP] Failed to create system server: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
	}
}

/// GET /api/v1/admin/mcp-servers/:id
pub async fn get_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match McpServer::find_scoped(&state.db, &id, None).await {
		Ok(Some(server)) => {
			let response = server.to_response_with_tools(&state.db, None).await;
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to get system server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PUT /api/v1/admin/mcp-servers/:id
pub async fn update_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateMcpServerRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let transport = match (req.transport.as_deref(), req.connection_config.as_ref()) {
		(Some(t), Some(config)) => match normalize_admin_transport(t, config) {
			Ok(norm) => Some(norm),
			Err(code) => return ErrorBuilder::new(code).build(),
		},
		(Some(t), None) => match normalize_admin_transport(t, &serde_json::json!({})) {
			Ok(norm) => Some(norm),
			Err(code) => return ErrorBuilder::new(code).build(),
		},
		(None, _) => None,
	};

	let updated = McpServer::update_scoped(&state.db, &id, None, req.name.as_deref(), transport, req.connection_config.as_ref(), req.is_enabled).await;

	match updated {
		Ok(Some(server)) => {
			state.mcp_pool.evict(&id).await;
			let response = server.to_response_with_tools(&state.db, None).await;
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to update system server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// DELETE /api/v1/admin/mcp-servers/:id
pub async fn delete_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match McpServer::delete_scoped(&state.db, &id, None).await {
		Ok(true) => {
			state.mcp_pool.evict(&id).await;
			(StatusCode::NO_CONTENT, "").into_response()
		}
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to delete system server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/admin/mcp-servers/:id/discover
pub async fn discover_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let server = match McpServer::find_scoped(&state.db, &id, None).await {
		Ok(Some(s)) => s,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to load system server for discovery: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	run_discovery(&state, &server, None).await
}

/// POST /api/v1/admin/mcp-servers/:id/health-check
pub async fn health_check(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let server = match McpServer::find_scoped(&state.db, &id, None).await {
		Ok(Some(s)) => s,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to load system server for health check: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	state.mcp_pool.evict(&server.id).await;
	let status = if server.discover().await.is_ok() { "healthy" } else { "unhealthy" };
	let _ = McpServer::set_health(&state.db, &server.id, status).await;

	ResponseBuilder::new(ResponseBody::Json(serde_json::json!({ "health_status": status }))).build()
}
