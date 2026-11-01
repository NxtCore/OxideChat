use crate::types::consts::{ADMIN_TOOLS_EDIT, ADMIN_TOOLS_VIEW};
use crate::types::tools::*;
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use crate::utils::tools::{ToolContext, ToolError, ToolExecutor};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

fn internal(what: &str, error: impl std::fmt::Display) -> ErrorCode {
	tracing::error!(%error, "{what}");
	ErrorCode::InternalError
}

pub async fn list_tools(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;
	json_ok(Tool::list_responses(&state.db, None).await?)
}

pub async fn get_tool(State(state): State<Arc<JobState>>, auth: AuthUser, Path(tool_id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;

	let tool = Tool::find_by_id_system(&state.db, &tool_id).await?.ok_or(ErrorCode::NotFound)?;
	let functions = ToolFunction::list_by_tool(&state.db, &tool_id).await.unwrap_or_default();
	json_ok(ToolResponse::from_tool_with_functions(tool, functions))
}

pub async fn create_tool(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<CreateToolRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let (tool, functions) = Tool::create_system(&state.db, &req).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(ToolResponse::from_tool_with_functions(tool, functions)))
		.status(StatusCode::CREATED)
		.build())
}

pub async fn update_tool(State(state): State<Arc<JobState>>, auth: AuthUser, Path(tool_id): Path<Uuid>, Json(req): Json<UpdateToolRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let (tool, functions) = Tool::update_system(&state.db, &tool_id, &req).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(ToolResponse::from_tool_with_functions(tool, functions))
}

pub async fn delete_tool(State(state): State<Arc<JobState>>, auth: AuthUser, Path(tool_id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	if !Tool::delete_system(&state.db, &tool_id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn get_tool_settings(State(state): State<Arc<JobState>>, auth: AuthUser, Path(tool_id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;
	json_ok(Tool::system_settings_for_admin(&state.db, &tool_id).await?)
}

/// PUT /api/v1/admin/tools/:id/settings
pub async fn set_tool_settings(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(tool_id): Path<Uuid>,
	Json(req): Json<SetToolSettingsRequest>,
) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	Tool::set_system_settings(&state.db, &tool_id, &req.settings).await?;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn upload_wasm(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<UploadWasmRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_EDIT).await?;

	let blob = base64::engine::general_purpose::STANDARD
		.decode(&req.content)
		.map_err(|_| ErrorCode::ValidationFailed)?;

	if blob.len() > 4 * 1024 * 1024 || blob.first_chunk::<4>() != Some(&[0x00, 0x61, 0x73, 0x6D]) {
		return Err(ErrorCode::ValidationFailed.into());
	}

	if req.filename.split('.').next_back() != Some("wasm") {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let mut hasher = Sha256::new();
	hasher.update(&blob);
	let hash = format!("{:x}", hasher.finalize());

	let existing = WasmBlob::find_by_hash(&state.db, &hash, None).await?;
	let created = existing.is_none();
	let blob_id = match existing {
		Some(id) => id,
		None => WasmBlob::create(&state.db, None, Some(&req.filename), Some("wasm"), &blob, blob.len() as i32, &hash).await?,
	};

	let response = UploadWasmResponse {
		blob_id,
		size_bytes: blob.len() as i32,
		sha256_hash: hash,
		compiled_from: Some("wasm".to_string()),
	};
	let status = if created { StatusCode::CREATED } else { StatusCode::OK };
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(status).build())
}

async fn run_wasm(state: &JobState, tool: &Tool, req: &TestToolRequest, ctx: &ToolContext) -> Result<Result<Value, ToolError>, ErrorCode> {
	let config: WasmSourceConfig = serde_json::from_value(tool.source_config.clone()).map_err(|e| internal("invalid WASM tool config", e))?;
	let blob = WasmBlob::find_by_id(&state.db, &config.wasm_blob_id).await.ok().flatten().ok_or(ErrorCode::NotFound)?;
	Ok(match crate::utils::tools::WasmExecutor::new(&blob, config.entrypoint) {
		Ok(executor) => executor.execute(req.input.clone(), ctx).await,
		Err(e) => Err(e),
	})
}

async fn run_http(state: &JobState, tool: &Tool, req: &TestToolRequest, ctx: &ToolContext) -> Result<Result<Value, ToolError>, ErrorCode> {
	let source_config = tool.revealed_source_config().map_err(|e| internal("failed to decrypt HTTP tool credentials", e))?;
	let config: HttpSourceConfig = serde_json::from_value(source_config).map_err(|e| internal("invalid HTTP tool config", e))?;

	let function_entrypoint = match &req.function_name {
		Some(fn_name) => ToolFunction::find_by_tool_and_name(&state.db, &tool.id, fn_name)
			.await
			.ok()
			.flatten()
			.and_then(|f| f.entrypoint),
		None => None,
	};

	let url = match &function_entrypoint {
		Some(entrypoint) if entrypoint.starts_with("http") => entrypoint.clone(),
		Some(entrypoint) => format!("{}{}", config.url.trim_end_matches('/'), entrypoint),
		None => config.url.clone(),
	};

	let http_config = crate::utils::tools::http::HttpConfig {
		url,
		method: config.method,
		headers: config.headers,
		body_template: config.body_template,
	};

	Ok(match crate::utils::tools::HttpExecutor::new(tool.name.clone(), http_config) {
		Ok(executor) => executor.execute(req.input.clone(), ctx).await,
		Err(e) => Err(e),
	})
}

async fn run_mcp(state: &JobState, tool: &Tool, req: &TestToolRequest) -> Result<Result<Value, ToolError>, AppError> {
	let config: McpSourceConfig = serde_json::from_value(tool.source_config.clone()).map_err(|e| internal("invalid MCP tool config", e))?;
	let server = McpServer::find_scoped(&state.db, &config.mcp_server_id, None).await?.ok_or(ErrorCode::NotFound)?;

	Ok(match server.get_client(&state.mcp_pool).await {
		Ok(client) => match client.call_tool(&config.tool_name, req.input.clone()).await {
			Ok(output) => Ok(output),
			Err(e) => {
				state.mcp_pool.evict(&server.id).await;
				Err(e)
			}
		},
		Err(e) => Err(e),
	})
}

pub async fn test_tool(State(state): State<Arc<JobState>>, auth: AuthUser, Path(tool_id): Path<Uuid>, Json(req): Json<TestToolRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TOOLS_VIEW).await?;

	let tool = Tool::find_by_id_system(&state.db, &tool_id).await?.ok_or(ErrorCode::NotFound)?;
	let tool_settings = Tool::effective_settings_for_user(&state.db, &tool_id, None)
		.await
		.map_err(|e| internal("failed to load tool settings", e))?;

	let ctx = ToolContext {
		user_id: None,
		settings: tool_settings,
		timeout_ms: Some(30000),
		function_name: req.function_name.clone(),
		db: Some(Arc::new(state.db.clone())),
	};

	let start = std::time::Instant::now();
	let result = match tool.source_kind {
		ToolSourceKind::Builtin => {
			let config: BuiltinSourceConfig = serde_json::from_value(tool.source_config.clone()).map_err(|e| internal("invalid builtin tool config", e))?;
			match crate::utils::tools::get_builtin_executor(&config.builtin_id) {
				Ok(executor) => executor.execute(req.input.clone(), &ctx).await,
				Err(e) => Err(e),
			}
		}
		ToolSourceKind::Wasm => run_wasm(&state, &tool, &req, &ctx).await?,
		ToolSourceKind::Http => run_http(&state, &tool, &req, &ctx).await?,
		ToolSourceKind::Mcp => run_mcp(&state, &tool, &req).await?,
	};
	let execution_ms = start.elapsed().as_millis() as i32;

	json_ok(match result {
		Ok(output) => TestToolResponse {
			success: true,
			output: Some(output),
			error: None,
			execution_ms,
		},
		Err(e) => TestToolResponse {
			success: false,
			output: None,
			error: Some(format!("{e}")),
			execution_ms,
		},
	})
}
