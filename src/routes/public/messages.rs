//! Message routes.
//!
//! Operations for chat messages.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::types::{ChatMessageResponse, Message, MessageListParams, SendMessageRequest, ToolExecutionResponse};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

/// GET /api/v1/chats/:chat_id/messages
///
/// Get messages for a chat (paginated).
pub async fn list_messages(
	State(state): State<Arc<AppState>>,
	cookies: Cookies,
	Path(chat_id): Path<Uuid>,
	Query(params): Query<MessageListParams>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let chat_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM chats WHERE id = $1 AND user_id = $2)")
		.bind(chat_id)
		.bind(user.id)
		.fetch_one(&state.db)
		.await;

	match chat_exists {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let messages = if let Some(before_id) = params.before {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages
			WHERE chat_id = $1 AND created_at < (SELECT created_at FROM messages WHERE id = $2)
			ORDER BY created_at DESC
			"#,
		)
		.bind(chat_id)
		.bind(before_id)
		.fetch_all(&state.db)
		.await
	} else if let Some(after_id) = params.after {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages
			WHERE chat_id = $1 AND created_at > (SELECT created_at FROM messages WHERE id = $2)
			ORDER BY created_at ASC
			"#,
		)
		.bind(chat_id)
		.bind(after_id)
		.fetch_all(&state.db)
		.await
	} else {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages
			WHERE chat_id = $1
			ORDER BY created_at ASC
			"#,
		)
		.bind(chat_id)
		.fetch_all(&state.db)
		.await
	};

	match messages {
		Ok(messages) => {
			let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();

			let tool_executions = if message_ids.is_empty() {
				vec![]
			} else {
				sqlx::query_as::<
					_,
					(
						Uuid,
						Uuid,
						String,
						serde_json::Value,
						Option<serde_json::Value>,
						Option<String>,
						Option<i32>,
						Option<Uuid>,
						Option<Uuid>,
						Option<String>,
					),
				>(
					r#"
					SELECT te.id, te.message_id, te.tool_call_id, te.input_args, te.output, te.error, te.execution_ms, te.tool_id, te.tool_function, t.name
					FROM tool_executions te
					LEFT JOIN tools t ON te.tool_id = t.id
					WHERE te.message_id = ANY($1)
					ORDER BY te.created_at ASC
					"#,
				)
				.bind(&message_ids)
				.fetch_all(&state.db)
				.await
				.unwrap_or_default()
			};

			let mut executions_by_message: std::collections::HashMap<Uuid, Vec<ToolExecutionResponse>> = std::collections::HashMap::new();
			for (id, message_id, tool_call_id, input_args, output, error, execution_ms, tool_id, tool_function, tool_name) in tool_executions {
				let response = ToolExecutionResponse {
					tool_call_id,
					tool_name: tool_name.unwrap_or_else(|| format!("tool_{}", id)),
					input_args,
					output,
					error,
					execution_ms,
					tool_id,
					tool_function,
				};
				executions_by_message.entry(message_id).or_default().push(response);
			}

			let responses: Vec<ChatMessageResponse> = messages
				.into_iter()
				.map(|m| {
					let msg_id = m.id;
					let response: ChatMessageResponse = m.into();
					let mut response = response;
					response.tool_calls = executions_by_message.remove(&msg_id);
					response
				})
				.collect();

			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[MESSAGES] Failed to list messages: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats/:chat_id/messages
///
/// Send a message. Returns the saved user message immediately.
/// AI response will be streamed separately via SSE.
pub async fn send_message(State(state): State<Arc<AppState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<SendMessageRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let chat = sqlx::query_as::<_, crate::types::Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
		.bind(chat_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let _chat = match chat {
		Ok(Some(chat)) => chat,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let reasoning_details = crate::types::ReasoningDetails {
		effort: req.reasoning_effort,
		budget_tokens: req.reasoning_budget_tokens,
	};
	let usage_details = crate::types::UsageDetails::default();
	let cost_details = crate::types::CostDetails::default();

	let message = sqlx::query_as::<_, Message>(
		r#"
		INSERT INTO messages (chat_id, role, content, model_id, reasoning_details, usage_details, cost_details)
		VALUES ($1, 'user', $2, $3, $4, $5, $6)
		RETURNING *
		"#,
	)
	.bind(chat_id)
	.bind(&req.content)
	.bind(req.model_id)
	.bind(sqlx::types::Json(reasoning_details))
	.bind(sqlx::types::Json(usage_details))
	.bind(sqlx::types::Json(cost_details))
	.fetch_one(&state.db)
	.await;

	let _ = sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
		.bind(chat_id)
		.execute(&state.db)
		.await;

	match message {
		Ok(msg) => {
			let response: ChatMessageResponse = msg.into();
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			eprintln!("[MESSAGES] Failed to save message: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
