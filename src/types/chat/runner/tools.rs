use crate::types::{Tool, ToolFunction};
use crate::utils::tools::{HttpExecutor, ToolContext, ToolExecutor, get_builtin_executor};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Returns `(mcp_server_id, mcp_tool_name, tool_id)` if the given tool name resolves to a
/// user-owned MCP tool that must be executed client-side. Returns `None` for
/// system/global tools and all non-MCP tool types.
pub(super) async fn get_client_tool_info(db: &PgPool, tool_name: &str, user_id: Uuid) -> Option<(Uuid, String, Uuid)> {
	use crate::types::tools::{McpSourceConfig, ToolSourceKind};

	let tool = Tool::find_enabled_by_name_for_user(db, tool_name, &user_id).await.ok().flatten()?;

	if tool.source_kind != ToolSourceKind::Mcp || tool.owner_id.is_none() {
		return None;
	}

	let config: McpSourceConfig = serde_json::from_value(tool.source_config).ok()?;
	Some((config.mcp_server_id, config.tool_name, tool.id))
}

pub(super) async fn execute_tool_by_name(
	db: &PgPool,
	mcp_pool: &crate::utils::tools::McpConnectionPool,
	user_id: Uuid,
	full_tool_name: &str,
	input: serde_json::Value,
) -> Result<crate::types::ToolExecutionResult, String> {
	use crate::types::ToolSourceKind;

	let mut tool: Option<Tool>;
	let mut function_name: Option<String> = None;
	let mut function_id: Option<Uuid> = None;

	tool = Tool::find_enabled_by_name_for_user(db, full_tool_name, &user_id)
		.await
		.map_err(|e| format!("Database error: {e}"))?;

	if tool.is_none() && full_tool_name.contains('_') {
		let underscores: Vec<_> = full_tool_name.match_indices('_').collect();
		for (pos, _) in underscores.iter().rev() {
			let potential_tool_name = &full_tool_name[..*pos];
			let potential_func_name = &full_tool_name[pos + 1..];

			let found = Tool::find_enabled_by_name_for_user(db, potential_tool_name, &user_id)
				.await
				.map_err(|e| format!("Database error: {e}"))?;

			if found.is_some() {
				tool = found;
				function_name = Some(potential_func_name.to_string());
				break;
			}
		}
	}

	let tool = tool.ok_or_else(|| format!("Tool '{}' not found", full_tool_name))?;

	let function_entrypoint = if let Some(fn_name) = &function_name {
		let func = ToolFunction::find_by_tool_and_name(db, &tool.id, fn_name)
			.await
			.map_err(|e| format!("Database error: {e}"))?
			.ok_or_else(|| format!("Function '{}' not found in tool '{}'", fn_name, tool.name))?;
		function_id = Some(func.id);
		func.entrypoint
	} else {
		None
	};

	let settings = Tool::effective_settings_for_user(db, &tool.id, Some(&user_id))
		.await
		.map_err(|error| format!("Failed to load tool settings: {error}"))?;

	let ctx = ToolContext {
		user_id: Some(user_id),
		settings,
		timeout_ms: Some(30000),
		function_name: function_name.map(|s| s.to_string()),
		db: Some(std::sync::Arc::new(db.clone())),
	};

	match tool.source_kind {
		ToolSourceKind::Builtin => {
			let builtin_id = tool
				.source_config
				.get("builtin_id")
				.and_then(|v| v.as_str())
				.ok_or("Missing builtin_id in source_config")?;

			let executor = get_builtin_executor(builtin_id).map_err(|e| format!("{:?}", e))?;
			let output = executor.execute(input, &ctx).await.map_err(|e| format!("{:?}", e))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		ToolSourceKind::Http => {
			let source_config = tool
				.revealed_source_config()
				.map_err(|error| format!("Failed to decrypt HTTP tool credentials: {error}"))?;
			let base_url = source_config.get("url").and_then(|v| v.as_str()).unwrap_or("");
			let url = if let Some(entrypoint) = &function_entrypoint {
				if entrypoint.starts_with("http") {
					entrypoint.clone()
				} else {
					format!("{}{}", base_url.trim_end_matches('/'), entrypoint)
				}
			} else {
				base_url.to_string()
			};

			let headers: HashMap<String, String> = source_config.get("headers").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

			let http_config = crate::utils::tools::http::HttpConfig {
				method: source_config.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_string(),
				url,
				headers,
				body_template: source_config.get("body_template").and_then(|v| v.as_str()).map(String::from),
			};

			let executor = HttpExecutor::new(full_tool_name.to_string(), http_config).map_err(|e| format!("{:?}", e))?;
			let output = executor.execute(input, &ctx).await.map_err(|e| format!("{:?}", e))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		ToolSourceKind::Mcp => {
			let output = execute_mcp_tool(db, mcp_pool, user_id, &tool, input).await.map_err(|e| format!("{e}"))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		ToolSourceKind::Wasm => Err(format!("WASM tools are not supported in chat execution: {:?}", tool.source_kind)),
	}
}

/// Execute an MCP-backed tool by resolving its server, obtaining a pooled client,
/// and calling the remote tool. On transport failure the cached client is evicted
/// so the next call reconnects.
async fn execute_mcp_tool(
	db: &PgPool,
	mcp_pool: &crate::utils::tools::McpConnectionPool,
	user_id: Uuid,
	tool: &Tool,
	input: serde_json::Value,
) -> Result<serde_json::Value, crate::utils::tools::ToolError> {
	use crate::types::McpSourceConfig;
	use crate::types::tools::McpServer;
	use crate::utils::tools::ToolError;

	let config: McpSourceConfig = serde_json::from_value(tool.source_config.clone()).map_err(|e| ToolError::McpError(format!("Invalid MCP source config: {e}")))?;

	let server = McpServer::find_owned_or_system(db, &config.mcp_server_id, &user_id)
		.await
		.map_err(|e| ToolError::McpError(format!("Database error: {e}")))?
		.ok_or_else(|| ToolError::NotFound(format!("MCP server {} not found", config.mcp_server_id)))?;

	if !server.is_enabled {
		return Err(ToolError::ExecutionFailed("MCP server is disabled".to_string()));
	}

	let client = server.get_client(mcp_pool).await?;
	match client.call_tool(&config.tool_name, input).await {
		Ok(output) => Ok(output),
		Err(e) => {
			mcp_pool.evict(&server.id).await;
			Err(e)
		}
	}
}
