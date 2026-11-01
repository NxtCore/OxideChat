use super::{CreateMcpServerRequest, McpHttpConfig, McpServer, McpServerResponse, McpStdioConfig, UpdateMcpServerRequest};
use crate::config::Config;
use crate::types::AppError;
use crate::utils::response::ErrorCode;
use crate::utils::tools::mcp::{McpUrlPolicy, is_remote_mcp_url_syntax_allowed, validate_remote_mcp_url};
use uuid::Uuid;

/// Who owns a set of MCP servers: the whole installation (admins) or a single user.
pub enum McpScope {
	System,
	User(Uuid),
}

fn normalize_remote_transport(transport: &str) -> Option<&'static str> {
	match transport.to_lowercase().as_str() {
		"http" | "streamable-http" | "streamable_http" => Some("http"),
		_ => None,
	}
}

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

	let admin_url_ok = serde_json::from_value::<McpHttpConfig>(connection_config.clone())
		.map(|c| is_remote_mcp_url_syntax_allowed(&c.url, McpUrlPolicy::TrustedAdmin))
		.unwrap_or(false);
	match normalize_remote_transport(transport) {
		Some(norm) if admin_url_ok => Ok(norm),
		_ => Err(ErrorCode::ValidationFailed),
	}
}

async fn validate_user_config(connection_config: &serde_json::Value) -> Result<(), ErrorCode> {
	let config = serde_json::from_value::<McpHttpConfig>(connection_config.clone()).map_err(|_| ErrorCode::ValidationFailed)?;
	validate_remote_mcp_url(&config.url, McpUrlPolicy::PublicOnly)
		.await
		.map_err(|_| ErrorCode::ValidationFailed)
}

impl McpScope {
	/// The owner stored on the server rows: `None` for system servers.
	#[must_use]
	pub fn owner_id(&self) -> Option<&Uuid> {
		match self {
			Self::System => None,
			Self::User(id) => Some(id),
		}
	}

	/// List the servers of this scope with their discovered tool names.
	///
	/// # Errors
	///
	/// Returns an error when the server query fails.
	pub async fn list(&self, pool: &sqlx::PgPool) -> Result<Vec<McpServerResponse>, sqlx::Error> {
		let servers = match self {
			Self::System => McpServer::list_system(pool).await?,
			Self::User(id) => McpServer::list_owned(pool, id).await?,
		};
		let mut responses = Vec::with_capacity(servers.len());
		for server in servers {
			responses.push(server.into_response_with_tools(pool, self.owner_id()).await);
		}
		Ok(responses)
	}

	/// Validate the request against this scope's transport policy and create the server.
	///
	/// # Errors
	///
	/// Returns a validation error for a bad name, transport or config, `Forbidden` for a
	/// disabled stdio transport, `AlreadyExists` for a duplicate name, or a database error.
	pub async fn create(&self, pool: &sqlx::PgPool, req: &CreateMcpServerRequest) -> Result<McpServerResponse, AppError> {
		if req.name.trim().is_empty() {
			return Err(ErrorCode::ValidationFailed.into());
		}
		let transport = match self {
			Self::System => normalize_admin_transport(&req.transport, &req.connection_config)?,
			Self::User(_) => {
				let transport = normalize_remote_transport(&req.transport).ok_or(ErrorCode::ValidationFailed)?;
				validate_user_config(&req.connection_config).await?;
				transport
			}
		};

		let server = McpServer::create(pool, self.owner_id(), req.name.trim(), transport, &req.connection_config, req.is_enabled)
			.await
			.map_err(|e| AppError::unique_or(e, ErrorCode::AlreadyExists))?;
		Ok(server.into_response_with_tools(pool, self.owner_id()).await)
	}

	/// Validate the request against this scope's transport policy and update the server.
	///
	/// # Errors
	///
	/// Returns a validation error for a bad transport or config, `NotFound` when the server
	/// is not in this scope, or a database error.
	pub async fn update(&self, pool: &sqlx::PgPool, id: &Uuid, req: &UpdateMcpServerRequest) -> Result<McpServerResponse, AppError> {
		let transport = match self {
			Self::System => match (req.transport.as_deref(), req.connection_config.as_ref()) {
				(Some(t), Some(config)) => Some(normalize_admin_transport(t, config)?),
				(Some(t), None) => Some(normalize_admin_transport(t, &serde_json::json!({}))?),
				(None, _) => None,
			},
			Self::User(_) => {
				let transport = match req.transport.as_deref() {
					Some(t) => Some(normalize_remote_transport(t).ok_or(ErrorCode::ValidationFailed)?),
					None => None,
				};
				if let Some(config) = &req.connection_config {
					validate_user_config(config).await?;
				}
				transport
			}
		};

		let server = McpServer::update_scoped(pool, id, self.owner_id(), req.name.as_deref(), transport, req.connection_config.as_ref(), req.is_enabled)
			.await?
			.ok_or(ErrorCode::NotFound)?;
		Ok(server.into_response_with_tools(pool, self.owner_id()).await)
	}
}
