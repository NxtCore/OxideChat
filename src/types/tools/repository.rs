use super::{
	McpHttpConfig, McpServer, McpServerResponse, McpSourceConfig, McpStdioConfig, Tool, ToolExecution, ToolFunction, ToolSourceKind, UserToolSettings, WasmBlob,
};
use crate::utils::tools::mcp::{McpToolInfo, McpUrlPolicy};
use crate::utils::tools::{McpClient, McpConnectionPool, ToolError};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

impl Tool {
	pub async fn set_system_settings(
		pool: &sqlx::PgPool,
		tool_id: &Uuid,
		settings: &serde_json::Value,
	) -> Result<(), super::ToolSettingsError> {
		let mut tx = pool.begin().await?;
		let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id IS NULL")
			.bind(tool_id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(super::ToolSettingsError::NotFound)?;

		if tool.source_kind == ToolSourceKind::Builtin && tool.source_config.get("builtin_id").and_then(serde_json::Value::as_str) == Some("imagegen") {
			let model_id = settings
				.get("image_model_id")
				.and_then(serde_json::Value::as_str)
				.and_then(|value| Uuid::parse_str(value).ok())
				.ok_or(super::ToolSettingsError::Invalid)?;
			let valid = sqlx::query_scalar::<_, bool>(
				r#"
				SELECT EXISTS (
					SELECT 1
					FROM models m
					JOIN providers p ON p.id = m.provider_id
					WHERE m.id = $1
					  AND m.is_enabled = true
					  AND p.is_enabled = true
					  AND p.kind IN ('OPENAI', 'OPENROUTER', 'GOOGLE')
					  AND EXISTS (
						  SELECT 1
						  FROM jsonb_array_elements_text(COALESCE(m.output_modalities, '[]'::jsonb)) AS modality(value)
						  WHERE LOWER(modality.value) = 'image'
					  )
				)
				"#,
			)
			.bind(model_id)
			.fetch_one(&mut *tx)
			.await?;
			if !valid {
				return Err(super::ToolSettingsError::Invalid);
			}
		}

		sqlx::query(
			r#"
			INSERT INTO user_tool_settings (user_id, tool_id, settings)
			VALUES (NULL, $1, $2)
			ON CONFLICT (tool_id) WHERE user_id IS NULL
			DO UPDATE SET settings = EXCLUDED.settings, updated_at = NOW()
			"#,
		)
		.bind(tool_id)
		.bind(settings)
		.execute(&mut *tx)
		.await?;
		tx.commit().await?;
		Ok(())
	}

	#[deprecated(note = "Use to_tool_specs with functions instead")]
	pub fn to_tool_spec(&self) -> omniference::types::ToolSpec {
		omniference::types::ToolSpec::JsonSchema {
			name: self.name.clone(),
			description: self.description.clone(),
			schema: self.input_schema.clone(),
			strict: Some(false),
		}
	}

	pub fn to_tool_specs(&self, functions: &[ToolFunction]) -> Vec<omniference::types::ToolSpec> {
		if functions.is_empty() {
			#[allow(deprecated)]
			return vec![self.to_tool_spec()];
		}

		functions
			.iter()
			.map(|f| {
				let name = if functions.len() == 1 {
					self.name.clone()
				} else {
					format!("{}_{}", self.name, f.name)
				};
				omniference::types::ToolSpec::JsonSchema {
					name,
					description: f.description.clone().or_else(|| self.description.clone()),
					schema: f.input_schema.clone(),
					strict: Some(false),
				}
			})
			.collect()
	}

	pub async fn list_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE owner_id IS NULL ORDER BY name")
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_id_system(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id IS NULL")
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		owner_id: Option<&Uuid>,
		name: &str,
		display_name: &str,
		description: Option<&str>,
		icon: Option<&str>,
		source_kind: &ToolSourceKind,
		source_config: &serde_json::Value,
		input_schema: &serde_json::Value,
		settings_schema: &serde_json::Value,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Tool>(
			r#"
			INSERT INTO tools (owner_id, name, display_name, description, icon,
			                   source_kind, source_config, input_schema, settings_schema, is_enabled)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
			RETURNING *
			"#,
		)
		.bind(owner_id)
		.bind(name)
		.bind(display_name)
		.bind(description)
		.bind(icon)
		.bind(source_kind)
		.bind(source_config)
		.bind(input_schema)
		.bind(settings_schema)
		.bind(is_enabled)
		.fetch_one(conn)
		.await
	}

	pub async fn update(
		conn: &mut sqlx::PgConnection,
		id: &Uuid,
		name: Option<&str>,
		display_name: Option<&str>,
		description: Option<&str>,
		icon: Option<&str>,
		source_config: Option<&serde_json::Value>,
		input_schema: Option<&serde_json::Value>,
		settings_schema: Option<&serde_json::Value>,
		is_enabled: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>(
			r#"
			UPDATE tools
			SET name = COALESCE($2, name),
			    display_name = COALESCE($3, display_name),
			    description = COALESCE($4, description),
			    icon = COALESCE($5, icon),
			    source_config = COALESCE($6, source_config),
			    input_schema = COALESCE($7, input_schema),
			    settings_schema = COALESCE($8, settings_schema),
			    is_enabled = COALESCE($9, is_enabled),
			    updated_at = NOW()
			WHERE id = $1
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(name)
		.bind(display_name)
		.bind(description)
		.bind(icon)
		.bind(source_config)
		.bind(input_schema)
		.bind(settings_schema)
		.bind(is_enabled)
		.fetch_optional(conn)
		.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM tools WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

	/// Fetch enabled tools by id that the user may use (owned by the user or global).
	pub async fn find_enabled_for_user(pool: &sqlx::PgPool, ids: &[Uuid], user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>(
			r#"
			SELECT id, owner_id, name, display_name, description, icon, source_kind,
			       source_config, input_schema, settings_schema, is_enabled, system_prompt, created_at, updated_at
			FROM tools
			WHERE id = ANY($1) AND is_enabled = true AND (owner_id = $2 OR owner_id IS NULL)
			"#,
		)
		.bind(ids)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	/// Look up an enabled tool by name that the user may use, preferring the
	/// user-owned tool over a global one on a name collision.
	pub async fn find_enabled_by_name_for_user(pool: &sqlx::PgPool, name: &str, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>(
			r#"
			SELECT id, owner_id, name, display_name, description, icon, source_kind,
			       source_config, input_schema, settings_schema, is_enabled, system_prompt, created_at, updated_at
			FROM tools
			WHERE name = $1 AND is_enabled = true AND (owner_id = $2 OR owner_id IS NULL)
			ORDER BY owner_id NULLS LAST
			LIMIT 1
			"#,
		)
		.bind(name)
		.bind(user_id)
		.fetch_optional(pool)
		.await
	}

	/// Display names of the generated tools for a given MCP server and owner scope.
	pub async fn names_for_mcp_server(pool: &sqlx::PgPool, server_id: &Uuid, owner_id: Option<&Uuid>) -> Result<Vec<String>, sqlx::Error> {
		let rows: Vec<(String,)> = sqlx::query_as(
			"SELECT display_name FROM tools WHERE source_kind = 'MCP' AND owner_id IS NOT DISTINCT FROM $2 AND source_config->>'mcp_server_id' = $1 ORDER BY display_name",
		)
		.bind(server_id.to_string())
		.bind(owner_id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|r| r.0).collect())
	}

	/// List enabled tools visible to the user in the chat tool selector
	/// (their own tools plus global tools).
	pub async fn list_for_user(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Tool>(
			r#"
			SELECT id, owner_id, name, display_name, description, icon, source_kind,
			       source_config, input_schema, settings_schema, is_enabled, system_prompt, created_at, updated_at
			FROM tools
			WHERE is_enabled = true AND (owner_id = $1 OR owner_id IS NULL)
			ORDER BY created_at DESC
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}
}

impl WasmBlob {
	pub async fn find_by_hash(pool: &sqlx::PgPool, sha256_hash: &str, owner_id: Option<&Uuid>) -> Result<Option<Uuid>, sqlx::Error> {
		let row: Option<(Uuid,)> = if let Some(owner) = owner_id {
			sqlx::query_as("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id = $2")
				.bind(sha256_hash)
				.bind(owner)
				.fetch_optional(pool)
				.await?
		} else {
			sqlx::query_as("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id IS NULL")
				.bind(sha256_hash)
				.fetch_optional(pool)
				.await?
		};
		Ok(row.map(|r| r.0))
	}

	pub async fn create(
		pool: &sqlx::PgPool,
		owner_id: Option<&Uuid>,
		original_filename: Option<&str>,
		compiled_from: Option<&str>,
		blob: &[u8],
		size_bytes: i32,
		sha256_hash: &str,
	) -> Result<Uuid, sqlx::Error> {
		let row: (Uuid,) = sqlx::query_as(
			r#"
			INSERT INTO wasm_blobs (owner_id, original_filename, compiled_from, blob, size_bytes, sha256_hash)
			VALUES ($1, $2, $3, $4, $5, $6)
			RETURNING id
			"#,
		)
		.bind(owner_id)
		.bind(original_filename)
		.bind(compiled_from)
		.bind(blob)
		.bind(size_bytes)
		.bind(sha256_hash)
		.fetch_one(pool)
		.await?;
		Ok(row.0)
	}

	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Vec<u8>>, sqlx::Error> {
		let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT blob FROM wasm_blobs WHERE id = $1").bind(id).fetch_optional(pool).await?;
		Ok(row.map(|r| r.0))
	}
}

impl ToolFunction {
	pub async fn list_by_tool(pool: &sqlx::PgPool, tool_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 ORDER BY sort_order, name")
			.bind(tool_id)
			.fetch_all(pool)
			.await
	}

	pub async fn list_by_tool_ids(pool: &sqlx::PgPool, tool_ids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = ANY($1) ORDER BY sort_order, name")
			.bind(tool_ids)
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_tool_and_name(pool: &sqlx::PgPool, tool_id: &Uuid, name: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 AND name = $2")
			.bind(tool_id)
			.bind(name)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		tool_id: &Uuid,
		name: &str,
		description: Option<&str>,
		input_schema: &serde_json::Value,
		entrypoint: Option<&str>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>(
			r#"
			INSERT INTO tool_functions (tool_id, name, description, input_schema, entrypoint)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING *
			"#,
		)
		.bind(tool_id)
		.bind(name)
		.bind(description)
		.bind(input_schema)
		.bind(entrypoint)
		.fetch_one(conn)
		.await
	}

	pub async fn update(
		conn: &mut sqlx::PgConnection,
		id: &Uuid,
		tool_id: &Uuid,
		name: &str,
		description: Option<&str>,
		input_schema: &serde_json::Value,
		entrypoint: Option<&str>,
	) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			UPDATE tool_functions
			SET name = $3, description = $4, input_schema = $5, entrypoint = $6
			WHERE id = $1 AND tool_id = $2
			"#,
		)
		.bind(id)
		.bind(tool_id)
		.bind(name)
		.bind(description)
		.bind(input_schema)
		.bind(entrypoint)
		.execute(conn)
		.await?;
		Ok(())
	}

	pub async fn delete(conn: &mut sqlx::PgConnection, id: &Uuid, tool_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM tool_functions WHERE id = $1 AND tool_id = $2")
			.bind(id)
			.bind(tool_id)
			.execute(conn)
			.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn delete_by_tool(conn: &mut sqlx::PgConnection, tool_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("DELETE FROM tool_functions WHERE tool_id = $1").bind(tool_id).execute(conn).await?;
		Ok(result.rows_affected())
	}
}

impl UserToolSettings {
	pub async fn find_by_tool_system(pool: &sqlx::PgPool, tool_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL")
			.bind(tool_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn find_by_tool_and_user(pool: &sqlx::PgPool, tool_id: &Uuid, user_id: Option<&Uuid>) -> Result<Option<Self>, sqlx::Error> {
		if let Some(uid) = user_id {
			sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id = $2")
				.bind(tool_id)
				.bind(uid)
				.fetch_optional(pool)
				.await
		} else {
			sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL")
				.bind(tool_id)
				.fetch_optional(pool)
				.await
		}
	}

	pub async fn find_effective_for_user(pool: &sqlx::PgPool, tool_id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserToolSettings>(
			r#"
			SELECT *
			FROM user_tool_settings
			WHERE tool_id = $1 AND (user_id = $2 OR user_id IS NULL)
			ORDER BY user_id NULLS LAST
			LIMIT 1
			"#,
		)
		.bind(tool_id)
		.bind(user_id)
		.fetch_optional(pool)
		.await
	}

	pub async fn upsert(pool: &sqlx::PgPool, tool_id: &Uuid, user_id: Option<&Uuid>, settings: &serde_json::Value) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			INSERT INTO user_tool_settings (tool_id, user_id, settings)
			VALUES ($1, $2, $3)
			ON CONFLICT (tool_id, user_id) WHERE user_id IS NOT NULL
			DO UPDATE SET settings = EXCLUDED.settings, updated_at = NOW()
			"#,
		)
		.bind(tool_id)
		.bind(user_id)
		.bind(settings)
		.execute(pool)
		.await?;
		Ok(())
	}
}

impl McpServer {
	pub async fn to_response_with_tools(self, pool: &sqlx::PgPool, owner_id: Option<&Uuid>) -> McpServerResponse {
		let names = Tool::names_for_mcp_server(pool, &self.id, owner_id).await.unwrap_or_default();
		let mut response = McpServerResponse::from_server(self, owner_id.is_some());
		response.discovered_tools = names;
		response
	}

	pub async fn list_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
			r#"
			SELECT id, owner_id, name, transport, connection_config, is_enabled,
			       last_health_check, health_status, created_at, updated_at
			FROM mcp_servers
			WHERE owner_id IS NULL
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	/// List the servers owned by a specific user (excludes global/system servers).
	pub async fn list_owned(pool: &sqlx::PgPool, owner_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
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
		.await
	}

	/// Fetch a server by id restricted to the given owner scope.
	///
	/// `owner_id = None` matches system (global) servers; `Some(uuid)` matches
	/// servers owned by that user.
	pub async fn find_scoped(pool: &sqlx::PgPool, id: &Uuid, owner_id: Option<&Uuid>) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
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
		.await
	}

	/// Fetch a server usable by a user for execution: their own or a global one.
	pub async fn find_owned_or_system(pool: &sqlx::PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
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
		.await
	}

	pub async fn create(
		pool: &sqlx::PgPool,
		owner_id: Option<&Uuid>,
		name: &str,
		transport: &str,
		connection_config: &serde_json::Value,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
			r#"
			INSERT INTO mcp_servers (owner_id, name, transport, connection_config, is_enabled)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING id, owner_id, name, transport, connection_config, is_enabled, last_health_check, health_status, created_at, updated_at
			"#,
		)
		.bind(owner_id)
		.bind(name)
		.bind(transport)
		.bind(connection_config)
		.bind(is_enabled)
		.fetch_one(pool)
		.await
	}

	pub async fn update_scoped(
		pool: &sqlx::PgPool,
		id: &Uuid,
		owner_id: Option<&Uuid>,
		name: Option<&str>,
		transport: Option<&str>,
		connection_config: Option<&serde_json::Value>,
		is_enabled: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>(
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
		.bind(connection_config)
		.bind(is_enabled)
		.fetch_optional(pool)
		.await
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

	/// Whether this server uses a server-side stdio transport.
	#[must_use]
	pub fn is_stdio(&self) -> bool {
		matches!(self.transport.to_lowercase().as_str(), "stdio")
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
				McpClient::new_http(self.name.clone(), config.url, config.headers, url_policy).await
			}
			"stdio" => {
				let config: McpStdioConfig =
					serde_json::from_value(self.connection_config.clone()).map_err(|e| ToolError::McpError(format!("Invalid stdio config: {e}")))?;
				McpClient::new_stdio(self.name.clone(), &config.command, &config.args, &config.env).await
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

impl ToolExecution {
	pub async fn create_for_message_batch(pool: &sqlx::PgPool, message_id: &Uuid, executions: &[crate::types::ToolExecutionInternal]) -> Result<(), sqlx::Error> {
		if executions.is_empty() {
			return Ok(());
		}

		let mut tx = pool.begin().await?;
		for exec in executions {
			sqlx::query(
				r#"
				INSERT INTO tool_executions (message_id, tool_call_id, input_args, output, error, execution_ms, tool_id, tool_function)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
				"#,
			)
			.bind(message_id)
			.bind(&exec.call_id)
			.bind(&exec.args)
			.bind(&exec.output)
			.bind(&exec.error)
			.bind(exec.execution_ms)
			.bind(exec.tool_id)
			.bind(exec.function_id)
			.execute(&mut *tx)
			.await?;
		}
		tx.commit().await
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		message_id: Option<&Uuid>,
		tool_id: Option<&Uuid>,
		tool_function: Option<&Uuid>,
		tool_call_id: &str,
		input_args: &serde_json::Value,
		output: Option<&serde_json::Value>,
		error: Option<&str>,
		execution_ms: Option<i32>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ToolExecution>(
			r#"
			INSERT INTO tool_executions (message_id, tool_id, tool_function, tool_call_id,
			                             input_args, output, error, execution_ms)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
			RETURNING *
			"#,
		)
		.bind(message_id)
		.bind(tool_id)
		.bind(tool_function)
		.bind(tool_call_id)
		.bind(input_args)
		.bind(output)
		.bind(error)
		.bind(execution_ms)
		.fetch_one(conn)
		.await
	}

	pub async fn list_by_message_ids(pool: &sqlx::PgPool, message_ids: &[Uuid]) -> Result<Vec<(Self, Option<String>)>, sqlx::Error> {
		let rows = sqlx::query(
			r#"
			SELECT te.*, t.name as tool_name
			FROM tool_executions te
			LEFT JOIN tools t ON te.tool_id = t.id
			WHERE te.message_id = ANY($1)
			ORDER BY te.created_at
			"#,
		)
		.bind(message_ids)
		.fetch_all(pool)
		.await?;

		Ok(rows
			.iter()
			.map(|row| {
				let exec = ToolExecution {
					id: row.get("id"),
					message_id: row.get("message_id"),
					tool_id: row.get("tool_id"),
					tool_call_id: row.get("tool_call_id"),
					input_args: row.get("input_args"),
					output: row.get("output"),
					error: row.get("error"),
					execution_ms: row.get("execution_ms"),
					created_at: row.get("created_at"),
				};
				let tool_name: Option<String> = row.get("tool_name");
				(exec, tool_name)
			})
			.collect())
	}
}
