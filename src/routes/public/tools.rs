//! Tool management routes.
//!
//! CRUD endpoints for managing user-defined tools.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
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
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

// ============= Tool CRUD =============

/// GET /api/v1/tools
/// List all tools available to the user (own + public)
pub async fn list_tools(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let tools = sqlx::query_as::<_, Tool>(
		r#"
        SELECT * FROM tools 
        WHERE owner_id = $1 OR is_public = true
        ORDER BY created_at DESC
        "#,
	)
	.bind(user.id)
	.fetch_all(&state.db)
	.await;

	match tools {
		Ok(tools) => {
			// Check which tools have user settings
			let tool_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();
			let settings_exist: Vec<Uuid> = sqlx::query_scalar("SELECT tool_id FROM user_tool_settings WHERE user_id = $1 AND tool_id = ANY($2)")
				.bind(user.id)
				.bind(&tool_ids)
				.fetch_all(&state.db)
				.await
				.unwrap_or_default();

			let responses: Vec<ToolResponse> = tools
				.into_iter()
				.map(|t| {
					let has_settings = settings_exist.contains(&t.id);
					let mut resp = ToolResponse::from(t);
					resp.has_user_settings = has_settings;
					resp
				})
				.collect();

			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[TOOLS] Failed to list tools: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// GET /api/v1/tools/:id
/// Get a specific tool
pub async fn get_tool(State(state): State<Arc<AppState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND (owner_id = $2 OR is_public = true)")
		.bind(tool_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	match tool {
		Ok(Some(tool)) => ResponseBuilder::new(ResponseBody::Json(ToolResponse::from(tool))).build(),
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to get tool: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/tools
/// Create a new tool
pub async fn create_tool(State(state): State<Arc<AppState>>, cookies: Cookies, Json(req): Json<CreateToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let tool = sqlx::query_as::<_, Tool>(
		r#"
        INSERT INTO tools (
            owner_id, name, display_name, description, icon,
            source_kind, source_config, input_schema, settings_schema,
            is_enabled, is_public
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING *
        "#,
	)
	.bind(user.id)
	.bind(&req.name)
	.bind(&req.display_name)
	.bind(&req.description)
	.bind(&req.icon)
	.bind(&req.source_kind)
	.bind(&req.source_config)
	.bind(&req.input_schema)
	.bind(&req.settings_schema)
	.bind(req.is_enabled)
	.bind(req.is_public)
	.fetch_one(&state.db)
	.await;

	match tool {
		Ok(tool) => ResponseBuilder::new(ResponseBody::Json(ToolResponse::from(tool))).status(StatusCode::CREATED).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to create tool: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PUT /api/v1/tools/:id
/// Update a tool
pub async fn update_tool(State(state): State<Arc<AppState>>, cookies: Cookies, Path(tool_id): Path<Uuid>, Json(req): Json<UpdateToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify ownership
	let existing = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND owner_id = $2")
		.bind(tool_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let existing = match existing {
		Ok(Some(t)) => t,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to fetch tool for update: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

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
            is_public = COALESCE($10, is_public),
            updated_at = NOW()
        WHERE id = $1
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
	.bind(req.is_public)
	.fetch_one(&state.db)
	.await;

	// Suppress warning
	let _ = existing;

	match tool {
		Ok(tool) => ResponseBuilder::new(ResponseBody::Json(ToolResponse::from(tool))).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to update tool: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// DELETE /api/v1/tools/:id
/// Delete a tool
pub async fn delete_tool(State(state): State<Arc<AppState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let result = sqlx::query("DELETE FROM tools WHERE id = $1 AND owner_id = $2")
		.bind(tool_id)
		.bind(user.id)
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

// ============= User Tool Settings =============

/// GET /api/v1/tools/:id/settings
/// Get user's settings for a tool
pub async fn get_tool_settings(State(state): State<Arc<AppState>>, cookies: Cookies, Path(tool_id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let settings = sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE user_id = $1 AND tool_id = $2")
		.bind(user.id)
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

/// PUT /api/v1/tools/:id/settings
/// Set user's settings for a tool (API keys, etc.)
pub async fn set_tool_settings(
	State(state): State<Arc<AppState>>,
	cookies: Cookies,
	Path(tool_id): Path<Uuid>,
	Json(req): Json<SetToolSettingsRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify tool exists and user has access
	let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND (owner_id = $2 OR is_public = true)")
		.bind(tool_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	match tool {
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TOOLS] Failed to verify tool: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		Ok(Some(_)) => {}
	}

	// Upsert settings
	let result = sqlx::query(
		r#"
        INSERT INTO user_tool_settings (user_id, tool_id, settings)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, tool_id) 
        DO UPDATE SET settings = $3, updated_at = NOW()
        "#,
	)
	.bind(user.id)
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

// ============= WASM Upload =============

/// POST /api/v1/tools/wasm/upload
/// Upload WASM binary or source file
pub async fn upload_wasm(State(state): State<Arc<AppState>>, cookies: Cookies, Json(req): Json<UploadWasmRequest>) -> impl IntoResponse {
	use base64::Engine;

	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Decode base64 content
	let blob = match base64::engine::general_purpose::STANDARD.decode(&req.content) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("[TOOLS] Invalid base64: {e}");
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
		}
	};

	// Determine source type from extension
	let extension = req.filename.split('.').last().unwrap_or("");
	let compiled_from = match extension {
		"wasm" => "wasm",
		"rs" => "rust",
		"js" | "ts" => "javascript",
		_ => return ErrorBuilder::new(ErrorCode::ValidationFailed).build(),
	};

	// For non-WASM, we'd need to compile here
	// For now, only accept pre-compiled WASM
	if compiled_from != "wasm" {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}

	// Calculate hash
	let mut hasher = Sha256::new();
	hasher.update(&blob);
	let hash = format!("{:x}", hasher.finalize());

	// Check for duplicate
	let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wasm_blobs WHERE sha256_hash = $1 AND owner_id = $2")
		.bind(&hash)
		.bind(user.id)
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

	// Store blob
	let blob_id = sqlx::query_scalar::<_, Uuid>(
		r#"
        INSERT INTO wasm_blobs (owner_id, original_filename, compiled_from, blob, size_bytes, sha256_hash)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
	)
	.bind(user.id)
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

// ============= Tool Testing =============

/// POST /api/v1/tools/:id/test
/// Test a tool with sample input
pub async fn test_tool(State(state): State<Arc<AppState>>, cookies: Cookies, Path(tool_id): Path<Uuid>, Json(req): Json<TestToolRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Fetch tool
	let tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = $1 AND (owner_id = $2 OR is_public = true)")
		.bind(tool_id)
		.bind(user.id)
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

	// Fetch user settings for this tool
	let user_settings = sqlx::query_as::<_, UserToolSettings>("SELECT * FROM user_tool_settings WHERE user_id = $1 AND tool_id = $2")
		.bind(user.id)
		.bind(tool_id)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	let ctx = crate::utils::tools::ToolContext {
		user_id: user.id,
		settings: user_settings.map(|s| s.settings).unwrap_or_default(),
		timeout_ms: Some(30000),
	};

	// Execute tool based on source kind
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

			// Fetch WASM blob
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

			let http_config = crate::utils::tools::http::HttpConfig {
				url: config.url,
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
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
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
