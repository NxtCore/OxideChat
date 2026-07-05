use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::tools::*;

use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{extract::State, response::IntoResponse};
use std::collections::HashMap;
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

pub async fn list_tools(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let tools = match Tool::list_for_user(&state.db, &user.id).await {
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
