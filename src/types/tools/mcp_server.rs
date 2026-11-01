use super::{McpDiscoveryResponse, McpHttpConfig, McpServer, McpServerResponse, McpSourceConfig, McpStdioConfig, Tool};
use crate::types::AppError;
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use crate::utils::encryption::{decrypt_object_values, encrypt_object_values};
use crate::utils::tools::mcp::{McpToolInfo, McpUrlPolicy};
use crate::utils::tools::{McpClient, McpConnectionPool, ToolError};
use serde_json::Value;
use axum::{http::StatusCode, response::Response};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

fn protect_connection_config(config: &Value, existing: Option<&Value>) -> Result<Value, crate::utils::encryption::EncryptionError> {
	let mut protected = config.clone();
	for key in ["headers", "env"] {
		if let Some(values) = config.get(key) {
			let previous = existing.and_then(|config| config.get(key));
			protected[key] = encrypt_object_values(values, previous)?;
		}
	}
	Ok(protected)
}

fn reveal_connection_config(config: &Value) -> Result<Value, crate::utils::encryption::EncryptionError> {
	let mut revealed = config.clone();
	for key in ["headers", "env"] {
		if let Some(values) = config.get(key) {
			revealed[key] = decrypt_object_values(values)?;
		}
	}
	Ok(revealed)
}

fn reveal_mcp_server(mut server: McpServer) -> Result<McpServer, sqlx::Error> {
	server.connection_config = reveal_connection_config(&server.connection_config).map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
	Ok(server)
}

impl McpServer {
	pub async fn into_response_with_tools(self, pool: &sqlx::PgPool, owner_id: Option<&Uuid>) -> McpServerResponse {
		let names = Tool::names_for_mcp_server(pool, &self.id, owner_id).await.unwrap_or_default();
		let mut response = McpServerResponse::from_server(self, owner_id.is_some());
		response.discovered_tools = names;
		response
	}

	pub async fn names_by_ids(pool: &sqlx::PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, String>, sqlx::Error> {
		let rows = sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM mcp_servers WHERE id = ANY($1)").bind(ids).fetch_all(pool).await?;
		Ok(rows.into_iter().collect())
	}

	pub async fn list_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		let servers = sqlx::query_as::<_, McpServer>(
			r#"
			SELECT id, owner_id, name, transport, connection_config, is_enabled,
			       last_health_check, health_status, created_at, updated_at
			FROM mcp_servers
			WHERE owner_id IS NULL
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await?;
		servers.into_iter().map(reveal_mcp_server).collect()
	}

	/// List the servers owned by a specific user (excludes global/system servers).
	pub async fn list_owned(pool: &sqlx::PgPool, owner_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		let servers = sqlx::query_as::<_, McpServer>(
			r#"
			SELECT id, owner_id, name, transport, connection_config, is_enabled,
			       last_health_check, health_status, created_at, updated_at
			FROM mcp_servers
			WHERE owner_id = $1
			ORDER BY name
			"#,
		)
		.bind(owner_id)
		.fetch_all(pool)
		.await?;
		servers.into_iter().map(reveal_mcp_server).collect()
	}

	/// Fetch a server by id restricted to the given owner scope.
	///
	/// `owner_id = None` matches system (global) servers; `Some(uuid)` matches
	/// servers owned by that user.
	pub async fn find_scoped(pool: &sqlx::PgPool, id: &Uuid, owner_id: Option<&Uuid>) -> Result<Option<Self>, sqlx::Error> {
		let server = sqlx::query_as::<_, McpServer>(
			r#"
			SELECT id, owner_id, name, transport, connection_config, is_enabled,
			       last_health_check, health_status, created_at, updated_at
			FROM mcp_servers
			WHERE id = $1 AND owner_id IS NOT DISTINCT FROM $2
			"#,
		)
		.bind(id)
		.bind(owner_id)
		.fetch_optional(pool)
		.await?;
		server.map(reveal_mcp_server).transpose()
	}

	/// Fetch a server usable by a user for execution: their own or a global one.
	pub async fn find_owned_or_system(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		let server = sqlx::query_as::<_, McpServer>(
			r#"
			SELECT id, owner_id, name, transport, connection_config, is_enabled,
			       last_health_check, health_status, created_at, updated_at
			FROM mcp_servers
			WHERE id = $1 AND (owner_id = $2 OR owner_id IS NULL)
			"#,
		)
		.bind(id)
		.bind(user_id)
		.fetch_optional(pool)
		.await?;
		server.map(reveal_mcp_server).transpose()
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn create(
		pool: &sqlx::PgPool,
		owner_id: Option<&Uuid>,
		name: &str,
		transport: &str,
		connection_config: &serde_json::Value,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		let protected = protect_connection_config(connection_config, None).map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
		let server = sqlx::query_as::<_, McpServer>(
			r#"
			INSERT INTO mcp_servers (owner_id, name, transport, connection_config, is_enabled)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING id, owner_id, name, transport, connection_config, is_enabled, last_health_check, health_status, created_at, updated_at
			"#,
		)
		.bind(owner_id)
		.bind(name)
		.bind(transport)
		.bind(&protected)
		.bind(is_enabled)
		.fetch_one(pool)
		.await?;
		reveal_mcp_server(server)
	}

	#[allow(clippy::too_many_arguments)]
	pub async fn update_scoped(
		pool: &sqlx::PgPool,
		id: &Uuid,
		owner_id: Option<&Uuid>,
		name: Option<&str>,
		transport: Option<&str>,
		connection_config: Option<&serde_json::Value>,
		is_enabled: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		let protected = if let Some(config) = connection_config {
			let Some(existing) = Self::find_scoped(pool, id, owner_id).await? else {
				return Ok(None);
			};
			Some(protect_connection_config(config, Some(&existing.connection_config)).map_err(|error| sqlx::Error::Protocol(error.to_string()))?)
		} else {
			None
		};
		let server = sqlx::query_as::<_, McpServer>(
			r#"
			UPDATE mcp_servers
			SET name = COALESCE($3, name),
			    transport = COALESCE($4, transport),
			    connection_config = COALESCE($5, connection_config),
			    is_enabled = COALESCE($6, is_enabled),
			    updated_at = NOW()
			WHERE id = $1 AND owner_id IS NOT DISTINCT FROM $2
			RETURNING id, owner_id, name, transport, connection_config, is_enabled, last_health_check, health_status, created_at, updated_at
			"#,
		)
		.bind(id)
		.bind(owner_id)
		.bind(name)
		.bind(transport)
		.bind(protected.as_ref())
		.bind(is_enabled)
		.fetch_optional(pool)
		.await?;
		server.map(reveal_mcp_server).transpose()
	}

	pub async fn delete_scoped(pool: &sqlx::PgPool, id: &Uuid, owner_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
		let mut tx = pool.begin().await?;

		sqlx::query("DELETE FROM tools WHERE source_kind = 'MCP' AND owner_id IS NOT DISTINCT FROM $1 AND source_config->>'mcp_server_id' = $2")
			.bind(owner_id)
			.bind(id.to_string())
			.execute(&mut *tx)
			.await?;

		let result = sqlx::query("DELETE FROM mcp_servers WHERE id = $1 AND owner_id IS NOT DISTINCT FROM $2")
			.bind(id)
			.bind(owner_id)
			.execute(&mut *tx)
			.await?;

		tx.commit().await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn set_health(pool: &sqlx::PgPool, id: &Uuid, status: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE mcp_servers SET health_status = $2, last_health_check = NOW(), updated_at = NOW() WHERE id = $1")
			.bind(id)
			.bind(status)
			.execute(pool)
			.await?;
		Ok(())
	}

	/// Open a fresh, initialized MCP client based on this server's transport.
	pub async fn connect(&self) -> Result<McpClient, ToolError> {
		match self.transport.to_lowercase().as_str() {
			"http" | "streamable-http" | "streamable_http" => {
				let config: McpHttpConfig =
					serde_json::from_value(self.connection_config.clone()).map_err(|e| ToolError::McpError(format!("Invalid HTTP config: {e}")))?;
				let url_policy = if self.owner_id.is_some() {
					McpUrlPolicy::PublicOnly
				} else {
					McpUrlPolicy::TrustedAdmin
				};
				McpClient::new_http(config.url, config.headers, url_policy).await
			}
			"stdio" => {
				let config: McpStdioConfig =
					serde_json::from_value(self.connection_config.clone()).map_err(|e| ToolError::McpError(format!("Invalid stdio config: {e}")))?;
				McpClient::new_stdio(&config.command, &config.args, &config.env).await
			}
			other => Err(ToolError::McpError(format!("Unsupported MCP transport: {other}"))),
		}
	}

	/// Obtain a shared client for this server from the pool, connecting on miss.
	pub async fn get_client(&self, mcp_pool: &McpConnectionPool) -> Result<Arc<McpClient>, ToolError> {
		if let Some(client) = mcp_pool.get(&self.id).await {
			return Ok(client);
		}
		let client = Arc::new(self.connect().await?);
		mcp_pool.insert(self.id, Arc::clone(&client)).await;
		Ok(client)
	}

	/// Connect and list the tools this server exposes.
	pub async fn discover(&self) -> Result<Vec<McpToolInfo>, ToolError> {
		let client = self.connect().await?;
		let tools = client.list_tools().await;
		let _ = client.close().await;
		tools
	}

	/// Discover this server's tools, store them for `owner_id` and answer with the
	/// discovery result, or with a 502 body when the server cannot be reached.
	pub async fn run_discovery(&self, pool: &sqlx::PgPool, mcp_pool: &McpConnectionPool, owner_id: Option<&Uuid>) -> Result<Response, AppError> {
		mcp_pool.evict(&self.id).await;
		let discovered = match self.discover().await {
			Ok(tools) => tools,
			Err(e) => {
				tracing::warn!(server_id = %self.id, error = %e, "MCP discovery failed");
				let _ = Self::set_health(pool, &self.id, "unhealthy").await;
				return Ok(ResponseBuilder::new(ResponseBody::Json(serde_json::json!({
					"error": ErrorCode::ServiceUnavailable.message(),
					"server_name": self.name,
				})))
				.status(StatusCode::BAD_GATEWAY)
				.build());
			}
		};

		self.sync_tools(pool, owner_id, &discovered).await?;
		let _ = Self::set_health(pool, &self.id, "healthy").await;
		Ok(ResponseBuilder::new(ResponseBody::Json(McpDiscoveryResponse::new(self.name.clone(), discovered))).build())
	}

	/// Replace the generated `Tool` records for this server with the freshly
	/// discovered tools, scoped to `owner_id` (`None` = system/global).
	///
	/// Each discovered MCP tool becomes one `Tool` row (`source_kind = MCP`)
	/// whose `input_schema` is passed to the model and whose `source_config`
	/// records the originating server and tool name.
	pub async fn sync_tools(&self, pool: &sqlx::PgPool, owner_id: Option<&Uuid>, discovered: &[McpToolInfo]) -> Result<Vec<Tool>, sqlx::Error> {
		let mut tx = pool.begin().await?;

		sqlx::query("DELETE FROM tools WHERE source_kind = 'MCP' AND owner_id IS NOT DISTINCT FROM $1 AND source_config->>'mcp_server_id' = $2")
			.bind(owner_id)
			.bind(self.id.to_string())
			.execute(&mut *tx)
			.await?;

		let mut created = Vec::with_capacity(discovered.len());
		for info in discovered {
			let tool_name = sanitize_tool_name(&format!("mcp_{}_{}", self.name, info.name));
			let source_config = serde_json::to_value(McpSourceConfig {
				mcp_server_id: self.id,
				tool_name: info.name.clone(),
			})
			.unwrap_or_else(|_| serde_json::json!({}));

			let tool = sqlx::query_as::<_, Tool>(
				r#"
				INSERT INTO tools (owner_id, name, display_name, description, icon,
				                   source_kind, source_config, input_schema, settings_schema, is_enabled)
				VALUES ($1, $2, $3, $4, NULL, 'MCP', $5, $6, '{}'::jsonb, true)
				RETURNING id, owner_id, name, display_name, description, icon, source_kind, source_config, input_schema, settings_schema, is_enabled, system_prompt, created_at, updated_at
				"#,
			)
			.bind(owner_id)
			.bind(&tool_name)
			.bind(&info.name)
			.bind(&info.description)
			.bind(&source_config)
			.bind(&info.input_schema)
			.fetch_one(&mut *tx)
			.await?;

			created.push(tool);
		}

		tx.commit().await?;
		Ok(created)
	}
}

/// Sanitize a name into a model-safe tool identifier: lowercase, only
/// `[a-z0-9_]`, collapsed underscores, capped at 100 characters.
fn sanitize_tool_name(raw: &str) -> String {
	let mut out = String::with_capacity(raw.len());
	let mut prev_underscore = false;
	for ch in raw.chars() {
		let mapped = if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '_' };
		if mapped == '_' {
			if prev_underscore {
				continue;
			}
			prev_underscore = true;
		} else {
			prev_underscore = false;
		}
		out.push(mapped);
	}
	let trimmed = out.trim_matches('_');
	let mut result: String = trimmed.chars().take(100).collect();
	if result.is_empty() {
		result.push_str("mcp_tool");
	}
	result
}
