//! User-owned MCP server management routes.
//!
//! Any authenticated user may register, discover, and remove their own remote
//! (Streamable HTTP) MCP servers. Server-side `stdio` transports are rejected
//! for user-owned servers — those are reserved for admins.

use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::tools::*;
use crate::utils::tools::mcp::McpToolInfo;
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

/// Normalize a user-supplied transport string, rejecting anything that is not the
/// supported remote transport (Streamable HTTP).
pub(crate) fn normalize_remote_transport(transport: &str) -> Option<&'static str> {
	match transport.to_lowercase().as_str() {
		"http" | "streamable-http" | "streamable_http" => Some("http"),
		_ => None,
	}
}

/// Validate that a remote transport's connection config contains a usable URL.
pub(crate) fn validate_remote_config(connection_config: &serde_json::Value) -> bool {
	serde_json::from_value::<McpHttpConfig>(connection_config.clone())
		.map(|c| !c.url.trim().is_empty())
		.unwrap_or(false)
}

/// Build a response for a server, populating its discovered tool names.
pub(crate) async fn server_response(db: &sqlx::PgPool, server: McpServer, owner_id: Option<&Uuid>) -> McpServerResponse {
	let names = Tool::names_for_mcp_server(db, &server.id, owner_id).await.unwrap_or_default();
	let mut response = McpServerResponse::from(server);
	response.discovered_tools = names;
	response
}

/// GET /api/v1/mcp-servers
pub async fn list_servers(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let servers = match McpServer::list_owned(&state.db, &user.id).await {
		Ok(s) => s,
		Err(e) => {
			eprintln!("[MCP] Failed to list servers: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut responses = Vec::with_capacity(servers.len());
	for server in servers {
		responses.push(server_response(&state.db, server, Some(&user.id)).await);
	}
	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}

/// POST /api/v1/mcp-servers
pub async fn create_server(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateMcpServerRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let Some(transport) = normalize_remote_transport(&req.transport) else {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	};
	if req.name.trim().is_empty() || !validate_remote_config(&req.connection_config) {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}

	match McpServer::create(&state.db, Some(&user.id), req.name.trim(), transport, &req.connection_config, req.is_enabled).await {
		Ok(server) => {
			let response = server_response(&state.db, server, Some(&user.id)).await;
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[MCP] Failed to create server: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
	}
}

/// GET /api/v1/mcp-servers/:id
pub async fn get_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match McpServer::find_scoped(&state.db, &id, Some(&user.id)).await {
		Ok(Some(server)) => {
			let response = server_response(&state.db, server, Some(&user.id)).await;
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to get server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PUT /api/v1/mcp-servers/:id
pub async fn update_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateMcpServerRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let transport = match req.transport.as_deref() {
		Some(t) => match normalize_remote_transport(t) {
			Some(norm) => Some(norm),
			None => return ErrorBuilder::new(ErrorCode::ValidationFailed).build(),
		},
		None => None,
	};
	if let Some(config) = &req.connection_config {
		if !validate_remote_config(config) {
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
		}
	}

	let updated = McpServer::update_scoped(
		&state.db,
		&id,
		Some(&user.id),
		req.name.as_deref(),
		transport,
		req.connection_config.as_ref(),
		req.is_enabled,
	)
	.await;

	match updated {
		Ok(Some(server)) => {
			state.mcp_pool.evict(&id).await;
			let response = server_response(&state.db, server, Some(&user.id)).await;
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to update server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// DELETE /api/v1/mcp-servers/:id
pub async fn delete_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match McpServer::delete_scoped(&state.db, &id, Some(&user.id)).await {
		Ok(true) => {
			state.mcp_pool.evict(&id).await;
			(StatusCode::NO_CONTENT, "").into_response()
		}
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to delete server: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/mcp-servers/:id/sync-tools
///
/// Accepts tool schemas discovered client-side (browser calling the local MCP)
/// and persists them as `Tool` records, exactly like server-side discover would.
/// Used for user-owned MCP servers whose URL is only reachable from the user's
/// machine (e.g. localhost).
pub async fn sync_tools_from_client(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<SyncMcpToolsRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let server = match McpServer::find_scoped(&state.db, &id, Some(&user.id)).await {
		Ok(Some(s)) => s,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to load server for sync: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let discovered: Vec<McpToolInfo> = req
		.tools
		.into_iter()
		.map(|t| McpToolInfo {
			name: t.name,
			description: t.description,
			input_schema: t.input_schema,
		})
		.collect();

	if let Err(e) = server.sync_tools(&state.db, Some(&user.id), &discovered).await {
		eprintln!("[MCP] Failed to sync client-discovered tools: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

	let _ = McpServer::set_health(&state.db, &server.id, "healthy").await;
	state.mcp_pool.evict(&server.id).await;

	let response = McpDiscoveryResponse {
		tools: discovered
			.into_iter()
			.map(|t| McpDiscoveredTool {
				name: t.name,
				description: t.description,
				input_schema: t.input_schema,
			})
			.collect(),
		server_name: server.name,
		server_version: None,
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}

/// POST /api/v1/mcp-servers/:id/discover
///
/// Connect to the server, list its tools, and (re)generate the user's `Tool`
/// records for it so they become available in the chat tool selector.
pub async fn discover_server(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let server = match McpServer::find_scoped(&state.db, &id, Some(&user.id)).await {
		Ok(Some(s)) => s,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MCP] Failed to load server for discovery: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	run_discovery(&state, &server, Some(&user.id)).await
}

/// Shared discovery logic used by both user and admin routes.
pub(crate) async fn run_discovery(state: &Arc<JobState>, server: &McpServer, owner_id: Option<&Uuid>) -> axum::response::Response {
	state.mcp_pool.evict(&server.id).await;
	let discovered = match server.discover().await {
		Ok(tools) => tools,
		Err(e) => {
			let _ = McpServer::set_health(&state.db, &server.id, "unhealthy").await;
			return ResponseBuilder::new(ResponseBody::Json(serde_json::json!({
				"error": e.to_string(),
				"server_name": server.name,
			})))
			.status(StatusCode::BAD_GATEWAY)
			.build();
		}
	};

	if let Err(e) = server.sync_tools(&state.db, owner_id, &discovered).await {
		eprintln!("[MCP] Failed to sync discovered tools: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}
	let _ = McpServer::set_health(&state.db, &server.id, "healthy").await;

	let response = McpDiscoveryResponse {
		tools: discovered
			.into_iter()
			.map(|t| McpDiscoveredTool {
				name: t.name,
				description: t.description,
				input_schema: t.input_schema,
			})
			.collect(),
		server_name: server.name.clone(),
		server_version: None,
	};
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}
