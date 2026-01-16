//! Message routes.
//!
//! Operations for chat messages.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::types::{
	BranchFromMessageRequest, BranchResponse, Chat, ChatMessageResponse, ChatResponse, EditMessageRequest, Message, MessageListParams, SendMessageRequest,
	SwitchForkRequest, ToolExecutionResponse,
};
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

	// Fetch only active fork messages
	let messages = if let Some(before_id) = params.before {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages
			WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at < (SELECT created_at FROM messages WHERE id = $2)
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
			WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at > (SELECT created_at FROM messages WHERE id = $2)
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
			WHERE chat_id = $1 AND is_active_fork = TRUE
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

			// Compute sibling counts for each unique (parent_id, role) pair
			// This ensures user messages only count user siblings, and assistant messages only count assistant siblings
			let sibling_counts: std::collections::HashMap<(Option<Uuid>, String), i64> = if message_ids.is_empty() {
				std::collections::HashMap::new()
			} else {
				// Query sibling counts grouped by parent_id and role
				let counts = sqlx::query_as::<_, (Option<Uuid>, String, i64)>(
					r#"
					SELECT parent_id, role, COUNT(*) as count
					FROM messages
					WHERE chat_id = $1
					GROUP BY parent_id, role
					"#,
				)
				.bind(chat_id)
				.fetch_all(&state.db)
				.await
				.unwrap_or_default();
				counts.into_iter().map(|(p, r, c)| ((p, r), c)).collect()
			};

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
					let parent_id = m.parent_id;
					let role = m.role.clone();
					let mut response: ChatMessageResponse = m.into();
					response.tool_calls = executions_by_message.remove(&msg_id);
					// Set computed sibling_count based on (parent_id, role) pair
					response.sibling_count = sibling_counts.get(&(parent_id, role)).copied().unwrap_or(1) as i32;
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

/// POST /api/v1/chats/:chat_id/messages/:message_id/edit
///
/// Edit a message, creating a new fork. The original message remains as a sibling.
pub async fn edit_message(
	State(state): State<Arc<AppState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<EditMessageRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify chat ownership
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

	// Get the original message
	let original = match sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
		.bind(message_id)
		.bind(chat_id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Get the next fork_index for siblings with same parent_id
	let next_fork_index: i32 = sqlx::query_scalar(r#"SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2"#)
		.bind(chat_id)
		.bind(original.parent_id)
		.fetch_one(&state.db)
		.await
		.unwrap_or(1);

	// Deactivate all siblings AND their descendants (entire old subtrees)
	let _ = sqlx::query(
		r#"
		WITH RECURSIVE descendants AS (
			-- Start with all siblings at this level
			SELECT id FROM messages 
			WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2
			UNION ALL
			-- Recursively get all descendants
			SELECT m.id FROM messages m
			INNER JOIN descendants d ON m.parent_id = d.id
			WHERE m.chat_id = $1
		)
		UPDATE messages SET is_active_fork = FALSE 
		WHERE id IN (SELECT id FROM descendants)
		"#,
	)
	.bind(chat_id)
	.bind(original.parent_id)
	.execute(&state.db)
	.await;

	// Create the new forked message
	let new_message = sqlx::query_as::<_, Message>(
		r#"
		INSERT INTO messages (chat_id, role, content, model_id, reasoning_details, usage_details, cost_details, parent_id, fork_index, is_active_fork)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, TRUE)
		RETURNING *
		"#,
	)
	.bind(chat_id)
	.bind(&original.role)
	.bind(&req.content)
	.bind(original.model_id)
	.bind(&original.reasoning_details)
	.bind(&original.usage_details)
	.bind(&original.cost_details)
	.bind(original.parent_id)
	.bind(next_fork_index)
	.fetch_one(&state.db)
	.await;

	let _ = sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
		.bind(chat_id)
		.execute(&state.db)
		.await;

	match new_message {
		Ok(msg) => {
			let mut response: ChatMessageResponse = msg.into();
			response.sibling_count = next_fork_index;
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			eprintln!("[MESSAGES] Failed to create fork: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/switch-fork
///
/// Switch to a different fork at the given message position.
/// This deactivates the entire old subtree and activates the new subtree.
pub async fn switch_fork(
	State(state): State<Arc<AppState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<SwitchForkRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify chat ownership
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

	// Get the message to find its parent_id
	let msg = match sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
		.bind(message_id)
		.bind(chat_id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Find the currently active sibling and deactivate it + all its descendants
	let _ = sqlx::query(
		r#"
		WITH RECURSIVE descendants AS (
			-- Start with the currently active sibling(s) at this level
			SELECT id FROM messages 
			WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND is_active_fork = TRUE
			UNION ALL
			-- Recursively get all descendants
			SELECT m.id FROM messages m
			INNER JOIN descendants d ON m.parent_id = d.id
			WHERE m.chat_id = $1
		)
		UPDATE messages SET is_active_fork = FALSE 
		WHERE id IN (SELECT id FROM descendants)
		"#,
	)
	.bind(chat_id)
	.bind(msg.parent_id)
	.execute(&state.db)
	.await;

	// Find the target sibling by fork_index
	let target = sqlx::query_as::<_, Message>(r#"SELECT * FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND fork_index = $3"#)
		.bind(chat_id)
		.bind(msg.parent_id)
		.bind(req.fork_index)
		.fetch_optional(&state.db)
		.await;

	let target_msg = match target {
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to find target fork: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Activate the target message and all its descendants
	let _ = sqlx::query(
		r#"
		WITH RECURSIVE descendants AS (
			-- Start with the target message
			SELECT id FROM messages WHERE id = $1
			UNION ALL
			-- Recursively get all descendants, but only follow the first (active) child at each level
			SELECT m.id FROM messages m
			INNER JOIN descendants d ON m.parent_id = d.id
			WHERE m.chat_id = $2 AND m.fork_index = 1
		)
		UPDATE messages SET is_active_fork = TRUE 
		WHERE id IN (SELECT id FROM descendants)
		"#,
	)
	.bind(target_msg.id)
	.bind(chat_id)
	.execute(&state.db)
	.await;

	let response: ChatMessageResponse = target_msg.into();
	ResponseBuilder::new(ResponseBody::Json(response)).build()
}

/// GET /api/v1/chats/:chat_id/messages/:message_id/siblings
///
/// Get all sibling messages (same parent_id) for fork navigation.
pub async fn get_siblings(State(state): State<Arc<AppState>>, cookies: Cookies, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify chat ownership
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

	// Get the message to find its parent_id
	let msg = match sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
		.bind(message_id)
		.bind(chat_id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Get all siblings
	let siblings = sqlx::query_as::<_, Message>(
		r#"
		SELECT * FROM messages 
		WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2
		ORDER BY fork_index ASC
		"#,
	)
	.bind(chat_id)
	.bind(msg.parent_id)
	.fetch_all(&state.db)
	.await;

	match siblings {
		Ok(msgs) => {
			let count = msgs.len() as i32;
			let responses: Vec<ChatMessageResponse> = msgs
				.into_iter()
				.map(|m| {
					let mut response: ChatMessageResponse = m.into();
					response.sibling_count = count;
					response
				})
				.collect();
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch siblings: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// DELETE /api/v1/chats/:chat_id/messages/:message_id/fork
///
/// Delete a fork and all its descendants.
pub async fn delete_fork(State(state): State<Arc<AppState>>, cookies: Cookies, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify chat ownership
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

	// Delete the message (CASCADE will delete descendants via parent_id FK)
	let result = sqlx::query("DELETE FROM messages WHERE id = $1 AND chat_id = $2")
		.bind(message_id)
		.bind(chat_id)
		.execute(&state.db)
		.await;

	match result {
		Ok(r) if r.rows_affected() > 0 => ResponseBuilder::new(ResponseBody::<()>::Empty).status(StatusCode::NO_CONTENT).build(),
		Ok(_) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to delete fork: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/branch
///
/// Create a new chat branched from a specific message.
/// - For assistant messages: copies all messages up to and including that point
/// - For user messages: copies all messages BEFORE that message, returns prefill data for composer
pub async fn branch_from_message(
	State(state): State<Arc<AppState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<BranchFromMessageRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Verify chat ownership and get source chat
	let source_chat = match sqlx::query_as::<_, Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
		.bind(chat_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(c)) => c,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Get the source message
	let source_message = match sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
		.bind(message_id)
		.bind(chat_id)
		.fetch_optional(&state.db)
		.await
	{
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Create the new branched chat
	let new_chat = match sqlx::query_as::<_, Chat>(
		r#"
		INSERT INTO chats (user_id, workspace_id, title, branched_from_chat_id, branched_from_message_id)
		VALUES ($1, $2, $3, $4, $5)
		RETURNING *
		"#,
	)
	.bind(user.id)
	.bind(req.workspace_id.or(source_chat.workspace_id))
	.bind(req.title.or(source_chat.title.clone()))
	.bind(chat_id)
	.bind(message_id)
	.fetch_one(&state.db)
	.await
	{
		Ok(c) => c,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to create branched chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let is_user_message = source_message.role == "user";

	// Get messages to copy (up to the branch point)
	// For user messages: copy messages BEFORE (not including) the source message
	// For assistant messages: copy messages up to and including the source message
	let messages_to_copy = if is_user_message {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages 
			WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at < $2
			ORDER BY created_at ASC
			"#,
		)
		.bind(chat_id)
		.bind(source_message.created_at)
		.fetch_all(&state.db)
		.await
	} else {
		sqlx::query_as::<_, Message>(
			r#"
			SELECT * FROM messages 
			WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at <= $2
			ORDER BY created_at ASC
			"#,
		)
		.bind(chat_id)
		.bind(source_message.created_at)
		.fetch_all(&state.db)
		.await
	};

	let messages_to_copy = match messages_to_copy {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch messages to copy: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Copy messages to new chat with proper parent_id mapping
	let mut old_to_new_id: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();

	for msg in &messages_to_copy {
		let new_parent_id = msg.parent_id.and_then(|pid| old_to_new_id.get(&pid).copied());

		let new_msg = sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, content_parts, reasoning_content, model_id, 
				cost_details, usage_details, reasoning_details, parent_id, fork_index, is_active_fork)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, TRUE)
			RETURNING *
			"#,
		)
		.bind(new_chat.id)
		.bind(&msg.role)
		.bind(&msg.content)
		.bind(&msg.content_parts)
		.bind(&msg.reasoning_content)
		.bind(msg.model_id)
		.bind(&msg.cost_details)
		.bind(&msg.usage_details)
		.bind(&msg.reasoning_details)
		.bind(new_parent_id)
		.fetch_one(&state.db)
		.await;

		match new_msg {
			Ok(m) => {
				old_to_new_id.insert(msg.id, m.id);
			}
			Err(e) => {
				eprintln!("[MESSAGES] Failed to copy message: {e}");
				// Continue with other messages
			}
		}
	}

	// Build response
	let message_count = messages_to_copy.len() as i64;
	let last_message_at = messages_to_copy.last().map(|m| m.created_at);

	let chat_response = ChatResponse {
		id: new_chat.id,
		workspace_id: new_chat.workspace_id,
		title: new_chat.title,
		is_pinned: new_chat.is_pinned,
		is_archived: new_chat.is_archived,
		branched_from_chat_id: new_chat.branched_from_chat_id,
		branched_from_message_id: new_chat.branched_from_message_id,
		message_count,
		last_message_at,
		created_at: new_chat.created_at,
		updated_at: new_chat.updated_at,
	};

	// For user messages, return prefill content
	let (prefill_content, prefill_parts) = if is_user_message {
		(Some(source_message.content.clone()), source_message.content_parts.clone())
	} else {
		(None, None)
	};

	let response = BranchResponse {
		chat: chat_response,
		prefill_content,
		prefill_parts,
	};

	ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
}
