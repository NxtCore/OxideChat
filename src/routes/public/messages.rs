use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{
	BranchFromMessageRequest, BranchResponse, Chat, ChatMessageResponse, ChatResponse, EditMessageRequest, Message, MessageListParams, ReasoningDetails,
	SendMessageRequest, SwitchForkRequest,
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
pub async fn list_messages(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(chat_id): Path<Uuid>,
	Query(params): Query<MessageListParams>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::exists_for_user(&state.db, &chat_id, &user.id).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let messages = match Message::list_by_chat(&state.db, &chat_id, params.before.as_ref(), params.after.as_ref()).await {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to list messages: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();

	let sibling_counts = match Message::sibling_counts_for_chat(&state.db, &chat_id).await {
		Ok(c) => c,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to get sibling counts: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut tool_executions = match Message::tool_executions_for_messages(&state.db, &message_ids).await {
		Ok(t) => t,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to get tool executions: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let responses: Vec<ChatMessageResponse> = messages
		.into_iter()
		.map(|m| {
			let msg_id = m.id;
			let parent_id = m.parent_id;
			let role = m.role.clone();
			let mut response = ChatMessageResponse::from(m);
			response.tool_calls = tool_executions.remove(&msg_id);
			response.sibling_count = sibling_counts.get(&(parent_id, role)).copied().unwrap_or(1) as i32;
			response
		})
		.collect();

	ResponseBuilder::new(ResponseBody::Json(responses)).build()
}

/// POST /api/v1/chats/:chat_id/messages
pub async fn send_message(State(state): State<Arc<JobState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<SendMessageRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::find_by_id_and_user(&state.db, &chat_id, &user.id).await {
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let reasoning_details = ReasoningDetails {
		effort: req.reasoning_effort,
		budget_tokens: req.reasoning_budget_tokens,
	};

	let message = Message::create_user_message(&state.db, &chat_id, &req.content, req.model_id.as_ref(), &reasoning_details).await;
	let _ = Chat::touch(&state.db, &chat_id).await;

	match message {
		Ok(msg) => {
			let response = ChatMessageResponse::from(msg);
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			eprintln!("[MESSAGES] Failed to save message: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/edit
pub async fn edit_message(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<EditMessageRequest>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::exists_for_user(&state.db, &chat_id, &user.id).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let original = match Message::find_by_id_and_chat(&state.db, &message_id, &chat_id).await {
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let new_message = match Message::create_fork(&state.db, &chat_id, &original, &req.content).await {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to create fork: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let _ = Chat::touch(&state.db, &chat_id).await;

	let sibling_count = match Message::sibling_count(&state.db, &chat_id, new_message.parent_id.as_ref()).await {
		Ok(c) => c,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to count sibling forks: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut response = ChatMessageResponse::from(new_message);
	response.sibling_count = sibling_count as i32;
	ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/switch-fork
pub async fn switch_fork(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<SwitchForkRequest>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::exists_for_user(&state.db, &chat_id, &user.id).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let msg = match Message::find_by_id_and_chat(&state.db, &message_id, &chat_id).await {
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	match Message::switch_to_fork(&state.db, &chat_id, msg.parent_id.as_ref(), req.fork_index).await {
		Ok(Some(target)) => ResponseBuilder::new(ResponseBody::Json(ChatMessageResponse::from(target))).build(),
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to switch fork: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// GET /api/v1/chats/:chat_id/messages/:message_id/siblings
pub async fn get_siblings(State(state): State<Arc<JobState>>, cookies: Cookies, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::exists_for_user(&state.db, &chat_id, &user.id).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	let msg = match Message::find_by_id_and_chat(&state.db, &message_id, &chat_id).await {
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	match Message::siblings(&state.db, &chat_id, msg.parent_id.as_ref()).await {
		Ok(siblings) => {
			let count = siblings.len() as i32;
			let responses: Vec<ChatMessageResponse> = siblings
				.into_iter()
				.map(|m| {
					let mut r = ChatMessageResponse::from(m);
					r.sibling_count = count;
					r
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
pub async fn delete_fork(State(state): State<Arc<JobState>>, cookies: Cookies, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::exists_for_user(&state.db, &chat_id, &user.id).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
		_ => {}
	}

	match Message::delete(&state.db, &message_id, &chat_id).await {
		Ok(true) => ResponseBuilder::new(ResponseBody::<()>::Empty).status(StatusCode::NO_CONTENT).build(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to delete fork: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/branch
pub async fn branch_from_message(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path((chat_id, message_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<BranchFromMessageRequest>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let source_chat = match Chat::find_by_id_and_user(&state.db, &chat_id, &user.id).await {
		Ok(Some(c)) => c,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to validate chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let source_message = match Message::find_by_id_and_chat(&state.db, &message_id, &chat_id).await {
		Ok(Some(m)) => m,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch message: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let new_chat = match Chat::create_branched(
		&state.db,
		&user.id,
		req.workspace_id.or(source_chat.workspace_id).as_ref(),
		req.title.as_deref().or(source_chat.title.as_deref()),
		&chat_id,
		&message_id,
	)
	.await
	{
		Ok(c) => c,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to create branched chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let is_user_message = source_message.role == "user";

	let messages_to_copy = if is_user_message {
		Message::list_active_before(&state.db, &chat_id, source_message.created_at).await
	} else {
		Message::list_active_up_to(&state.db, &chat_id, source_message.created_at).await
	};

	let messages_to_copy = match messages_to_copy {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[MESSAGES] Failed to fetch messages to copy: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	if let Err(e) = Message::copy_to_chat(&state.db, &new_chat.id, &messages_to_copy).await {
		eprintln!("[MESSAGES] Failed to copy messages: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

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

	let (prefill_content, prefill_parts) = if is_user_message {
		(Some(source_message.content), source_message.content_parts)
	} else {
		(None, None)
	};

	ResponseBuilder::new(ResponseBody::Json(BranchResponse {
		chat: chat_response,
		prefill_content,
		prefill_parts,
	}))
	.status(StatusCode::CREATED)
	.build()
}
