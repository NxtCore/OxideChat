use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::consts::{ADMIN_TOOLS_EDIT, ADMIN_TOOLS_VIEW};
use crate::types::tools::*;
use crate::utils::auth::has_permission;
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

pub async fn list_tools(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !has_permission(&state.db, &user.id, ADMIN_TOOLS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let tools = match sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE owner_id IS NULL AND is_public = true ORDER BY created_at DESC")
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

	let mut functions_by_tool: std::collections::HashMap<Uuid, Vec<ToolFunction>> = std::collections::HashMap::new();
	for f in functions {
		functions_by_tool.entry(f.tool_id).or_default().push(f);
	}

	let responses: Vec<ToolResponse> = tools
		.into_iter()
		.map(|t| {
			let funcs = functions_by_tool.remove(&t.id).unwrap_or_default();
			ToolResponse::from_tool_with_functions(t, funcs)
		})
		.collect();

	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}
