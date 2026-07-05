use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::consts::{ADMIN_TOOLS_EDIT, ADMIN_TOOLS_VIEW};
use crate::types::tools::*;

use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use crate::utils::tools::ToolExecutor;
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

pub async fn list_tools(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let tools = match sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE owner_id IS NULL ORDER BY created_at DESC")
		.fetch_all(&state.db)
		.await
	{
		Ok(t) => t,
		Err(e) => {
			eprintln!("[TOOLS] Failed to list tools: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let tool_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();
	let functions: Vec<ToolFunction> = if tool_ids.is_empty() {
		vec![]
	} else {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = ANY($1) ORDER BY sort_order, created_at")
			.bind(&tool_ids)
			.fetch_all(&state.db)
			.await
			.unwrap_or_default()
	};

	let mut functions_by_tool: HashMap<Uuid, Vec<ToolFunction>> = HashMap::new();
	for f in functions {
		functions_by_tool.entry(f.tool_id).or_default().push(f);
	}

	let mut responses: Vec<ToolResponse> = tools
		.into_iter()
		.map(|t| {
			let funcs = functions_by_tool.remove(&t.id).unwrap_or_default();
			ToolResponse::from_tool_with_functions(t, funcs)
		})
		.collect();

	let server_ids: Vec<Uuid> = responses
		.iter()
		.filter_map(|r| r.mcp_server_id)
		.collect::<std::collections::HashSet<_>>()
		.into_iter()
		.collect();

	if !server_ids.is_empty() {
		let server_names: HashMap<Uuid, String> =
			sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM mcp_servers WHERE id = ANY($1)")
				.bind(&server_ids)
				.fetch_all(&state.db)
				.await
				.unwrap_or_default()
				.into_iter()
				.collect();

		for r in &mut responses {
			r.mcp_server_name = r.mcp_server_id.and_then(|id| server_names.get(&id).cloned());
		}
	}

	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}

pub async fn get_tool(State(state): State<Arc<JobState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await;

	match tool {
		Ok(Some(tool)) => {
			let functions = sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 ORDER BY sort_order, created_at")
				.bind(tool_id)
				.fetch_all(&state.db)
				.await
				.unwrap_or_default();

			ResponseBuilder::new(ResponseBody::Json(ToolResponse::from_tool_with_functions(tool, functions))).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to get tool: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn create_tool(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let legacy_schema = req.input_schema.clone().unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

	let tool = sqlx::query_as::<_, Tool>(
		r#"
        INSERT INTO tools (
            owner_id, name, display_name, description, icon,
            source_kind, source_config, input_schema, settings_schema,
            is_enabled
        )
        VALUES (NULL, $1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#,
	)
	.bind(&req.name)
	.bind(&req.display_name)
	.bind(&req.description)
	.bind(&req.icon)
	.bind(&req.source_kind)
	.bind(&req.source_config)
	.bind(&legacy_schema)
	.bind(&req.settings_schema)
	.bind(req.is_enabled)
	.fetch_one(&state.db)
	.await;

	let tool = match tool {
		Ok(t) => t,
		Err(e) => {
			eprintln!("[TOOLS] Failed to create tool: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut functions: Vec<ToolFunction> = vec![];

	if !req.functions.is_empty() {
		for (idx, func) in req.functions.iter().enumerate() {
			let f = sqlx::query_as::<_, ToolFunction>(
				r#"
				INSERT INTO tool_functions (tool_id, name, description, input_schema, entrypoint, sort_order)
				VALUES ($1, $2, $3, $4, $5, $6)
				RETURNING *
				"#,
			)
			.bind(tool.id)
			.bind(&func.name)
			.bind(&func.description)
			.bind(&func.input_schema)
			.bind(&func.entrypoint)
			.bind(idx as i32)
			.fetch_one(&state.db)
			.await;

			if let Ok(f) = f {
				functions.push(f);
			}
		}
	} else if req.input_schema.is_some() {
		let f = sqlx::query_as::<_, ToolFunction>(
			r#"
			INSERT INTO tool_functions (tool_id, name, description, input_schema, entrypoint, sort_order)
			VALUES ($1, $2, $3, $4, NULL, 0)
			RETURNING *
			"#,
		)
		.bind(tool.id)
		.bind(&req.name) // Use tool name as function name
		.bind(&req.description)
		.bind(&legacy_schema)
		.fetch_one(&state.db)
		.await;

		if let Ok(f) = f {
			functions.push(f);
		}
	}

	ResponseBuilder::new(ResponseBody::Json(ToolResponse::from_tool_with_functions(tool, functions)))
		.status(StatusCode::CREATED)
		.build()
}

pub async fn update_tool(State(state): State<Arc<JobState>>, cookies: Cookies, Path(tool_id): Path<Uuid>, Json(req): Json<UpdateToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let existing = match sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(existing) => existing,
		Err(e) => {
			eprintln!("[TOOLS] Failed to fetch tool for update: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};
	if existing.is_none() {
		return ErrorBuilder::new(ErrorCode::NotFound).build();
	}

	let tool = sqlx::query_as::<_, Tool>(
		r#"
        UPDATE tools SET
            name = COALESCE($2, name),
            display_name = COALESCE($3, display_name),
            description = COALESCE($4, description),
            icon = COALESCE($5, icon),
            source_config = COALESCE($6, source_config),
            input_schema = COALESCE($7, input_schema),
            settings_schema = COALESCE($8, settings_schema),
            is_enabled = COALESCE($9, is_enabled),
            updated_at = NOW()
        WHERE id = $1 AND owner_id IS NULL
        RETURNING *
        "#,
	)
	.bind(tool_id)
	.bind(&req.name)
	.bind(&req.display_name)
	.bind(&req.description)
	.bind(&req.icon)
	.bind(&req.source_config)
	.bind(&req.input_schema)
	.bind(&req.settings_schema)
	.bind(req.is_enabled)
	.fetch_one(&state.db)
	.await;

	let tool = match tool {
		Ok(t) => t,
		Err(e) => {
			eprintln!("[TOOLS] Failed to update tool: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	if let Some(ref delete_ids) = req.delete_function_ids {
		for func_id in delete_ids {
			let _ = sqlx::query("DELETE FROM tool_functions WHERE id = $1 AND tool_id = $2")
				.bind(func_id)
				.bind(tool_id)
				.execute(&state.db)
				.await;
		}
	}

	if let Some(ref funcs) = req.functions {
		for (idx, func) in funcs.iter().enumerate() {
			if let Some(func_id) = func.id {
				// Update existing function
				let _ = sqlx::query(
					r#"
					UPDATE tool_functions SET
						name = $2,
						description = $3,
						input_schema = $4,
						entrypoint = $5,
						sort_order = $6
					WHERE id = $1 AND tool_id = $7
					"#,
				)
				.bind(func_id)
				.bind(&func.name)
				.bind(&func.description)
				.bind(&func.input_schema)
				.bind(&func.entrypoint)
				.bind(idx as i32)
				.bind(tool_id)
				.execute(&state.db)
				.await;
			} else {
				// Insert new function
				let _ = sqlx::query(
					r#"
					INSERT INTO tool_functions (tool_id, name, description, input_schema, entrypoint, sort_order)
					VALUES ($1, $2, $3, $4, $5, $6)
					"#,
				)
				.bind(tool_id)
				.bind(&func.name)
				.bind(&func.description)
				.bind(&func.input_schema)
				.bind(&func.entrypoint)
				.bind(idx as i32)
				.execute(&state.db)
				.await;
			}
		}
	}

	let functions = sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 ORDER BY sort_order, created_at")
		.bind(tool_id)
		.fetch_all(&state.db)
		.await
		.unwrap_or_default();

	ResponseBuilder::new(ResponseBody::Json(ToolResponse::from_tool_with_functions(tool, functions))).build()
}

pub async fn delete_tool(State(state): State<Arc<JobState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let result = sqlx::query("DELETE FROM tools WHERE id = $1 AND owner_id IS NULL")
		.bind(tool_id)
		.execute(&state.db)
		.await;

	match result {
		Ok(res) if res.rows_affected() > 0 => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(_) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to delete tool: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn get_tool_settings(State(state): State<Arc<JobState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let settings = sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await;

	match settings {
		Ok(Some(s)) => ResponseBuilder::new(ResponseBody::Json(s.settings)).build(),
		Ok(None) => ResponseBuilder::new(ResponseBody::Json(serde_json::json!({}))).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to get tool settings: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PUT /api/v1/admin/tools/:id/settings
pub async fn set_tool_settings(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(tool_id): Path<Uuid>,
	Json(req): Json<SetToolSettingsRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let tool_exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM tools WHERE id = $1 AND owner_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	if tool_exists.is_none() {
		return ErrorBuilder::new(ErrorCode::NotFound).build();
	}

	let result = sqlx::query(
		r#"
        INSERT INTO user_tool_settings (user_id, tool_id, settings)
        VALUES (NULL, $1, $2)
        ON CONFLICT (tool_id) WHERE user_id IS NULL
        DO UPDATE SET settings = $2, updated_at = NOW()
        "#,
	)
	.bind(tool_id)
	.bind(&req.settings)
	.execute(&state.db)
	.await;

	match result {
		Ok(_) => (StatusCode::NO_CONTENT, "").into_response(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to save tool settings: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn upload_wasm(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<UploadWasmRequest>) -> impl IntoResponse {
	use base64::Engine;

	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let blob = match base64::engine::general_purpose::STANDARD.decode(&req.content) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("[TOOLS] Invalid base64: {e}");
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
		}
	};

	let extension = req.filename.split('.').last().unwrap_or("");
	let compiled_from = match extension {
		"wasm" => "wasm",
		"rs" => "rust",
		"js" | "ts" => "javascript",
		_ => return ErrorBuilder::new(ErrorCode::ValidationFailed).build(),
	};

	if compiled_from != "wasm" {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}

	let mut hasher = Sha256::new();
	hasher.update(&blob);
	let hash = format!("{:x}", hasher.finalize());

	let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id IS NULL")
		.bind(&hash)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	if let Some(existing_id) = existing {
		return ResponseBuilder::new(ResponseBody::Json(UploadWasmResponse {
			blob_id: existing_id,
			size_bytes: blob.len() as i32,
			sha256_hash: hash,
			compiled_from: Some(compiled_from.to_string()),
		}))
		.build();
	}

	let blob_id = sqlx::query_scalar::<_, Uuid>(
		r#"
        INSERT INTO wasm_blobs (owner_id, original_filename, compiled_from, blob, size_bytes, sha256_hash)
        VALUES (NULL, $1, $2, $3, $4, $5)
        RETURNING id
        "#,
	)
	.bind(&req.filename)
	.bind(compiled_from)
	.bind(&blob)
	.bind(blob.len() as i32)
	.bind(&hash)
	.fetch_one(&state.db)
	.await;

	match blob_id {
		Ok(blob_id) => ResponseBuilder::new(ResponseBody::Json(UploadWasmResponse {
			blob_id,
			size_bytes: blob.len() as i32,
			sha256_hash: hash,
			compiled_from: Some(compiled_from.to_string()),
		}))
		.status(StatusCode::CREATED)
		.build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to store WASM: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn test_tool(State(state): State<Arc<JobState>>, cookies: Cookies, Path(tool_id): Path<Uuid>, Json(req): Json<TestToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await;

	let tool = match tool {
		Ok(Some(t)) => t,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to fetch tool: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let tool_settings = sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE tool_id = $1 AND user_id IS NULL")
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	let ctx = crate::utils::tools::ToolContext {
		user_id: None,
		settings: tool_settings.map(|s| s.settings).unwrap_or_default(),
		timeout_ms: Some(30000),
		function_name: req.function_name.clone(),
		db: Some(std::sync::Arc::new(state.db.clone())),
	};

	let start = std::time::Instant::now();
	let result = match tool.source_kind {
		ToolSourceKind::Builtin => {
			let config: BuiltinSourceConfig = match serde_json::from_value(tool.source_config.clone()) {
				Ok(c) => c,
				Err(e) => {
					eprintln!("[TOOLS] Invalid builtin config: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};

			match crate::utils::tools::get_builtin_executor(&config.builtin_id) {
				Ok(executor) => executor.execute(req.input.clone(), &ctx).await,
				Err(e) => Err(e),
			}
		}
		ToolSourceKind::Wasm => {
			let config: WasmSourceConfig = match serde_json::from_value(tool.source_config.clone()) {
				Ok(c) => c,
				Err(e) => {
					eprintln!("[TOOLS] Invalid WASM config: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};

			let blob: Result<Vec<u8>, _> = sqlx::query_scalar("SELECT blob FROM wasm_blobs WHERE id = $1")
				.bind(config.wasm_blob_id)
				.fetch_one(&state.db)
				.await;

			match blob {
				Ok(blob) => match crate::utils::tools::WasmExecutor::new(tool.name.clone(), &blob, config.entrypoint) {
					Ok(executor) => executor.execute(req.input.clone(), &ctx).await,
					Err(e) => Err(e),
				},
				Err(e) => {
					eprintln!("[TOOLS] WASM blob not found: {e}");
					return ErrorBuilder::new(ErrorCode::NotFound).build();
				}
			}
		}
		ToolSourceKind::Http => {
			let config: HttpSourceConfig = match serde_json::from_value(tool.source_config.clone()) {
				Ok(c) => c,
				Err(e) => {
					eprintln!("[TOOLS] Invalid HTTP config: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};

			let function_entrypoint = if let Some(ref fn_name) = req.function_name {
				sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 AND name = $2")
					.bind(tool_id)
					.bind(fn_name)
					.fetch_optional(&state.db)
					.await
					.ok()
					.flatten()
					.and_then(|f| f.entrypoint)
			} else {
				None
			};

			let url = if let Some(entrypoint) = &function_entrypoint {
				if entrypoint.starts_with("http") {
					entrypoint.clone()
				} else {
					format!("{}{}", config.url.trim_end_matches('/'), entrypoint)
				}
			} else {
				config.url.clone()
			};

			let http_config = crate::utils::tools::http::HttpConfig {
				url,
				method: config.method,
				headers: config.headers,
				body_template: config.body_template,
			};

			match crate::utils::tools::HttpExecutor::new(tool.name.clone(), http_config) {
				Ok(executor) => executor.execute(req.input.clone(), &ctx).await,
				Err(e) => Err(e),
			}
		}
		ToolSourceKind::Mcp => {
			let config: McpSourceConfig = match serde_json::from_value(tool.source_config.clone()) {
				Ok(c) => c,
				Err(e) => {
					eprintln!("[TOOLS] Invalid MCP config: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};

			let server = match McpServer::find_scoped(&state.db, &config.mcp_server_id, None).await {
				Ok(Some(s)) => s,
				Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
				Err(e) => {
					eprintln!("[TOOLS] Failed to load MCP server: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};

			match server.get_client(&state.mcp_pool).await {
				Ok(client) => match client.call_tool(&config.tool_name, req.input.clone()).await {
					Ok(output) => Ok(output),
					Err(e) => {
						state.mcp_pool.evict(&server.id).await;
						Err(e)
					}
				},
				Err(e) => Err(e),
			}
		}
	};
	let execution_ms = start.elapsed().as_millis() as i32;

	match result {
		Ok(output) => ResponseBuilder::new(ResponseBody::Json(TestToolResponse {
			success: true,
			output: Some(output),
			error: None,
			execution_ms,
		}))
		.build(),
		Err(e) => ResponseBuilder::new(ResponseBody::Json(TestToolResponse {
			success: false,
			output: None,
			error: Some(format!("{e}")),
			execution_ms,
		}))
		.build(),
	}
}
