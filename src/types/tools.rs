//! Tool types for OxideChat API.
//!
//! Types for managing custom tools, MCP servers, and tool executions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// Tool source kind enum matching the database enum
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "tool_source_kind", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolSourceKind {
	Builtin,
	Wasm,
	Mcp,
	Http,
}

impl ToolSourceKind {
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Builtin => "BUILTIN",
			Self::Wasm => "WASM",
			Self::Mcp => "MCP",
			Self::Http => "HTTP",
		}
	}

	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"BUILTIN" => Some(Self::Builtin),
			"WASM" => Some(Self::Wasm),
			"MCP" => Some(Self::Mcp),
			"HTTP" => Some(Self::Http),
			_ => None,
		}
	}
}

/// WASM blob storage database row
#[derive(Debug, Clone, FromRow)]
pub struct WasmBlob {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub original_filename: Option<String>,
	pub compiled_from: Option<String>,
	pub blob: Vec<u8>,
	pub size_bytes: i32,
	pub sha256_hash: String,
	pub created_at: DateTime<Utc>,
}

/// Tool database row
#[derive(Debug, Clone, FromRow)]
pub struct Tool {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub source_config: serde_json::Value,
	pub input_schema: serde_json::Value,
	pub settings_schema: serde_json::Value,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Tool function database row - one tool can have multiple functions
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ToolFunction {
	pub id: Uuid,
	pub tool_id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
	pub sort_order: i32,
	pub created_at: DateTime<Utc>,
}

/// User-provided tool settings database row
#[derive(Debug, Clone, FromRow)]
pub struct UserToolSettings {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub tool_id: Uuid,
	pub settings: serde_json::Value,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// MCP Server database row
#[derive(Debug, Clone, FromRow)]
pub struct McpServer {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
	pub is_enabled: bool,
	pub last_health_check: Option<DateTime<Utc>>,
	pub health_status: Option<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Tool execution audit log database row
#[derive(Debug, Clone, FromRow)]
pub struct ToolExecution {
	pub id: Uuid,
	pub message_id: Option<Uuid>,
	pub tool_id: Option<Uuid>,
	pub tool_call_id: String,
	pub input_args: serde_json::Value,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: Option<i32>,
	pub created_at: DateTime<Utc>,
}

/// WASM source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSourceConfig {
	pub wasm_blob_id: Uuid,
	pub entrypoint: String,
	pub compiled_from: Option<String>,
}

/// MCP source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSourceConfig {
	pub mcp_server_id: Uuid,
	pub tool_name: String,
}

/// HTTP source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSourceConfig {
	pub url: String,
	pub method: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
	pub body_template: Option<String>,
}

/// Builtin source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinSourceConfig {
	pub builtin_id: String,
}

/// MCP stdio transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioConfig {
	pub command: String,
	#[serde(default)]
	pub args: Vec<String>,
	#[serde(default)]
	pub env: std::collections::HashMap<String, String>,
}

/// MCP SSE transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSseConfig {
	pub url: String,
	#[serde(default)]
	pub headers: std::collections::HashMap<String, String>,
}

/// Request to create a tool function
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateToolFunctionRequest {
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
}

/// Request to create a new tool
#[derive(Debug, Deserialize)]
pub struct CreateToolRequest {
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub source_config: serde_json::Value,
	#[serde(default)]
	pub input_schema: Option<serde_json::Value>,
	#[serde(default)]
	pub functions: Vec<CreateToolFunctionRequest>,
	#[serde(default)]
	pub settings_schema: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

fn default_true() -> bool {
	true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateToolFunctionRequest {
	pub id: Option<Uuid>,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
}

/// Request to update an existing tool
#[derive(Debug, Deserialize)]
pub struct UpdateToolRequest {
	pub name: Option<String>,
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_config: Option<serde_json::Value>,
	pub input_schema: Option<serde_json::Value>,
	pub functions: Option<Vec<UpdateToolFunctionRequest>>,
	pub delete_function_ids: Option<Vec<Uuid>>,
	pub settings_schema: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

/// Request to upload WASM source or binary
#[derive(Debug, Deserialize)]
pub struct UploadWasmRequest {
	pub filename: String,
	pub content: String,
	#[serde(default)]
	pub force_compile: bool,
}

/// Request to create MCP server
#[derive(Debug, Deserialize)]
pub struct CreateMcpServerRequest {
	pub name: String,
	pub transport: String,
	pub connection_config: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

/// Request to update MCP server
#[derive(Debug, Deserialize)]
pub struct UpdateMcpServerRequest {
	pub name: Option<String>,
	pub transport: Option<String>,
	pub connection_config: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

/// Request to set user tool settings
#[derive(Debug, Deserialize)]
pub struct SetToolSettingsRequest {
	pub settings: serde_json::Value,
}

/// Request to test tool execution
#[derive(Debug, Deserialize)]
pub struct TestToolRequest {
	pub input: serde_json::Value,
	#[serde(default)]
	pub function_name: Option<String>,
}

/// Tool function response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	pub entrypoint: Option<String>,
	pub sort_order: i32,
}

impl From<ToolFunction> for ToolFunctionResponse {
	fn from(f: ToolFunction) -> Self {
		Self {
			id: f.id,
			name: f.name,
			description: f.description,
			input_schema: f.input_schema,
			entrypoint: f.entrypoint,
			sort_order: f.sort_order,
		}
	}
}

/// Tool response
#[derive(Debug, Serialize)]
pub struct ToolResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub display_name: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub source_kind: ToolSourceKind,
	pub input_schema: serde_json::Value,
	pub functions: Vec<ToolFunctionResponse>,
	pub settings_schema: serde_json::Value,
	pub is_enabled: bool,
	pub has_user_settings: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl ToolResponse {
	pub fn from_tool_with_functions(t: Tool, functions: Vec<ToolFunction>) -> Self {
		Self {
			id: t.id,
			owner_id: t.owner_id,
			name: t.name,
			display_name: t.display_name,
			description: t.description,
			icon: t.icon,
			source_kind: t.source_kind,
			input_schema: t.input_schema,
			functions: functions.into_iter().map(ToolFunctionResponse::from).collect(),
			settings_schema: t.settings_schema,
			is_enabled: t.is_enabled,
			has_user_settings: false,
			created_at: t.created_at,
			updated_at: t.updated_at,
		}
	}
}

impl From<Tool> for ToolResponse {
	fn from(t: Tool) -> Self {
		Self {
			id: t.id,
			owner_id: t.owner_id,
			name: t.name.clone(),
			display_name: t.display_name,
			description: t.description.clone(),
			icon: t.icon,
			source_kind: t.source_kind,
			input_schema: t.input_schema.clone(),
			functions: vec![],
			settings_schema: t.settings_schema,
			is_enabled: t.is_enabled,
			has_user_settings: false,
			created_at: t.created_at,
			updated_at: t.updated_at,
		}
	}
}

/// MCP Server response
#[derive(Debug, Serialize)]
pub struct McpServerResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub transport: String,
	pub is_enabled: bool,
	pub last_health_check: Option<DateTime<Utc>>,
	pub health_status: Option<String>,
	pub discovered_tools: Vec<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl From<McpServer> for McpServerResponse {
	fn from(s: McpServer) -> Self {
		Self {
			id: s.id,
			owner_id: s.owner_id,
			name: s.name,
			transport: s.transport,
			is_enabled: s.is_enabled,
			last_health_check: s.last_health_check,
			health_status: s.health_status,
			discovered_tools: vec![],
			created_at: s.created_at,
			updated_at: s.updated_at,
		}
	}
}

/// WASM upload response
#[derive(Debug, Serialize)]
pub struct UploadWasmResponse {
	pub blob_id: Uuid,
	pub size_bytes: i32,
	pub sha256_hash: String,
	pub compiled_from: Option<String>,
}

/// Tool test response
#[derive(Debug, Serialize)]
pub struct TestToolResponse {
	pub success: bool,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: i32,
}

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
	pub tool_id: Uuid,
	pub function_id: Option<Uuid>,
	pub output: serde_json::Value,
}

/// MCP discovery response
#[derive(Debug, Serialize)]
pub struct McpDiscoveryResponse {
	pub tools: Vec<McpDiscoveredTool>,
	pub server_name: String,
	pub server_version: Option<String>,
}

/// Discovered MCP tool
#[derive(Debug, Serialize)]
pub struct McpDiscoveredTool {
	pub name: String,
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
}

/// Tool execution log response (for audit/logging views)
#[derive(Debug, Serialize)]
pub struct ToolExecutionLogResponse {
	pub id: Uuid,
	pub tool_id: Option<Uuid>,
	pub tool_name: Option<String>,
	pub tool_call_id: String,
	pub input_args: serde_json::Value,
	pub output: Option<serde_json::Value>,
	pub error: Option<String>,
	pub execution_ms: Option<i32>,
	pub created_at: DateTime<Utc>,
}

impl Tool {
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
}

impl crate::types::BaseType for WasmBlob {}

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
		let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT blob FROM wasm_blobs WHERE id = $1")
			.bind(id)
			.fetch_optional(pool)
			.await?;
		Ok(row.map(|r| r.0))
	}
}

impl crate::types::BaseType for Tool {}

impl Tool {
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
}

impl crate::types::BaseType for ToolFunction {}

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
		let result = sqlx::query("DELETE FROM tool_functions WHERE tool_id = $1")
			.bind(tool_id)
			.execute(conn)
			.await?;
		Ok(result.rows_affected())
	}
}

impl crate::types::BaseType for UserToolSettings {}

impl UserToolSettings {
	pub async fn find_by_tool_system(pool: &sqlx::PgPool, tool_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserToolSettings>(
			"SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL",
		)
		.bind(tool_id)
		.fetch_optional(pool)
		.await
	}

	pub async fn find_by_tool_and_user(pool: &sqlx::PgPool, tool_id: &Uuid, user_id: Option<&Uuid>) -> Result<Option<Self>, sqlx::Error> {
		if let Some(uid) = user_id {
			sqlx::query_as::<_, UserToolSettings>(
				"SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id = $2",
			)
			.bind(tool_id)
			.bind(uid)
			.fetch_optional(pool)
			.await
		} else {
			sqlx::query_as::<_, UserToolSettings>(
				"SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL",
			)
			.bind(tool_id)
			.fetch_optional(pool)
			.await
		}
	}

	pub async fn upsert(
		pool: &sqlx::PgPool,
		tool_id: &Uuid,
		user_id: Option<&Uuid>,
		settings: &serde_json::Value,
	) -> Result<(), sqlx::Error> {
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

impl crate::types::BaseType for McpServer {}

impl McpServer {
	pub async fn list_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, McpServer>("SELECT * FROM mcp_servers WHERE owner_id IS NULL ORDER BY name")
			.fetch_all(pool)
			.await
	}
}

impl crate::types::BaseType for ToolExecution {}

impl ToolExecution {
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

	pub async fn list_by_message_ids(
		pool: &sqlx::PgPool,
		message_ids: &[Uuid],
	) -> Result<Vec<(Self, Option<String>)>, sqlx::Error> {
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
