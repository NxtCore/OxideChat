use crate::types::{
	AppError, AuthUser, BranchFromMessageRequest, BranchResponse, Chat, ChatMessageResponse, ChatResponse, EditMessageRequest, JobState, Message, MessageListParams, ReasoningDetails,
	SendMessageRequest, SwitchForkRequest, json_ok,
};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

async fn require_chat(state: &JobState, chat_id: &Uuid, user: &AuthUser) -> Result<(), AppError> {
	if Chat::exists_for_user(&state.db, chat_id, &user.id).await? {
		Ok(())
	} else {
		Err(ErrorCode::NotFound.into())
	}
}

async fn find_message(state: &JobState, chat_id: &Uuid, message_id: &Uuid) -> Result<Message, AppError> {
	Ok(Message::find_by_id_and_chat(&state.db, message_id, chat_id).await?.ok_or(ErrorCode::NotFound)?)
}

/// GET /`api/v1/chats/:chat_id/messages`
pub async fn list_messages(State(state): State<Arc<JobState>>, user: AuthUser, Path(chat_id): Path<Uuid>, Query(params): Query<MessageListParams>) -> Result<Response, AppError> {
	require_chat(&state, &chat_id, &user).await?;
	let messages = Message::list_by_chat(&state.db, &chat_id, params.before.as_ref(), params.after.as_ref()).await?;
	json_ok(ChatMessageResponse::list_for(&state.db, &chat_id, messages).await?)
}

/// POST /`api/v1/chats/:chat_id/messages`
pub async fn send_message(State(state): State<Arc<JobState>>, user: AuthUser, Path(chat_id): Path<Uuid>, Json(req): Json<SendMessageRequest>) -> Result<Response, AppError> {
	Chat::find_by_id_and_user(&state.db, &chat_id, &user.id).await?.ok_or(ErrorCode::NotFound)?;

	let reasoning_details = ReasoningDetails {
		effort: req.reasoning_effort,
		budget_tokens: req.reasoning_budget_tokens,
	};

	let message = Message::create_user_message(&state.db, &chat_id, &req.content, req.model_id.as_ref(), &reasoning_details).await;
	let _ = Chat::touch(&state.db, &chat_id).await;

	let response = ChatMessageResponse::from(message?);
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// POST /`api/v1/chats/:chat_id/messages/:message_id/edit`
pub async fn edit_message(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, message_id)): Path<(Uuid, Uuid)>, Json(req): Json<EditMessageRequest>) -> Result<Response, AppError> {
	require_chat(&state, &chat_id, &user).await?;
	let original = find_message(&state, &chat_id, &message_id).await?;
	let new_message = Message::create_fork(&state.db, &chat_id, &original, &req.content).await?;
	let _ = Chat::touch(&state.db, &chat_id).await;
	let sibling_count = Message::sibling_count(&state.db, &chat_id, new_message.parent_id.as_ref()).await?;

	let mut response = ChatMessageResponse::from(new_message);
	response.sibling_count = sibling_count as i32;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// POST /api/v1/chats/:chat_id/messages/:message_id/switch-fork
pub async fn switch_fork(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, message_id)): Path<(Uuid, Uuid)>, Json(req): Json<SwitchForkRequest>) -> Result<Response, AppError> {
	require_chat(&state, &chat_id, &user).await?;
	let msg = find_message(&state, &chat_id, &message_id).await?;
	let target = Message::switch_to_fork(&state.db, &chat_id, msg.parent_id.as_ref(), req.fork_index).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(ChatMessageResponse::from(target))
}

/// GET /`api/v1/chats/:chat_id/messages/:message_id/siblings`
pub async fn get_siblings(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	require_chat(&state, &chat_id, &user).await?;
	let msg = find_message(&state, &chat_id, &message_id).await?;
	let siblings = Message::siblings(&state.db, &chat_id, msg.parent_id.as_ref()).await?;
	let count = siblings.len() as i32;
	let responses: Vec<ChatMessageResponse> = siblings
		.into_iter()
		.map(|m| {
			let mut r = ChatMessageResponse::from(m);
			r.sibling_count = count;
			r
		})
		.collect();
	json_ok(responses)
}

/// DELETE /`api/v1/chats/:chat_id/messages/:message_id/fork`
pub async fn delete_fork(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, message_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	require_chat(&state, &chat_id, &user).await?;
	if Message::delete(&state.db, &message_id, &chat_id).await? {
		Ok(ResponseBuilder::new(ResponseBody::<()>::Empty).status(StatusCode::NO_CONTENT).build())
	} else {
		Err(ErrorCode::NotFound.into())
	}
}

/// POST /`api/v1/chats/:chat_id/messages/:message_id/branch`
pub async fn branch_from_message(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, message_id)): Path<(Uuid, Uuid)>, Json(req): Json<BranchFromMessageRequest>) -> Result<Response, AppError> {
	let source_chat = Chat::find_by_id_and_user(&state.db, &chat_id, &user.id).await?.ok_or(ErrorCode::NotFound)?;
	let source_message = find_message(&state, &chat_id, &message_id).await?;

	let new_chat = Chat::create_branched(
		&state.db,
		&user.id,
		req.workspace_id.or(source_chat.workspace_id).as_ref(),
		req.title.as_deref().or(source_chat.title.as_deref()),
		&chat_id,
		&message_id,
	)
	.await?;

	let is_user_message = source_message.role == "user";

	let messages_to_copy = if is_user_message {
		Message::list_active_before(&state.db, &chat_id, source_message.created_at).await?
	} else {
		Message::list_active_up_to(&state.db, &chat_id, source_message.created_at).await?
	};

	Message::copy_to_chat(&state.db, &new_chat.id, &messages_to_copy).await?;

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

	Ok(ResponseBuilder::new(ResponseBody::Json(BranchResponse {
		chat: chat_response,
		prefill_content,
		prefill_parts,
	}))
	.status(StatusCode::CREATED)
	.build())
}
