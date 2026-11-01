use crate::types::{AppError, AuthUser, BaseType, Chat, ChatListParams, ChatMessageResponse, ChatResponse, ChatWithMessagesResponse, CreateChatRequest, JobState, Message, Page, PageQuery, UpdateChatRequest, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

fn chat_response_with_stats(chat: &Chat, message_count: i64, last_message_at: Option<DateTime<Utc>>) -> ChatResponse {
	ChatResponse {
		id: chat.id,
		workspace_id: chat.workspace_id,
		title: chat.title.clone(),
		is_pinned: chat.is_pinned,
		is_archived: chat.is_archived,
		branched_from_chat_id: chat.branched_from_chat_id,
		branched_from_message_id: chat.branched_from_message_id,
		message_count,
		last_message_at,
		created_at: chat.created_at,
		updated_at: chat.updated_at,
	}
}

async fn build_chat_response(pool: &PgPool, chat: &Chat) -> Result<ChatResponse, sqlx::Error> {
	let (message_count, last_message_at) = Chat::message_stats(pool, &chat.id).await?;
	Ok(chat_response_with_stats(chat, message_count, last_message_at))
}

/// GET /api/v1/chats
pub async fn list_chats(State(state): State<Arc<JobState>>, user: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ChatListParams>) -> Result<Response, AppError> {
	let pagination = Chat::pagination(&page);
	let mut chats = Chat::list_by_user(&state.db, &user.id, params.workspace_id.as_ref(), params.include_archived, pagination.limit, pagination.offset).await?;
	let has_more = chats.len() > pagination.page_size;
	chats.truncate(pagination.page_size);
	let chat_ids: Vec<Uuid> = chats.iter().map(|c| c.id).collect();
	let stats = Chat::message_stats_batch(&state.db, &chat_ids).await?;
	let responses = chats
		.iter()
		.map(|chat| {
			let (count, last) = stats.get(&chat.id).copied().unwrap_or((0, None));
			chat_response_with_stats(chat, count, last)
		})
		.collect::<Vec<_>>();
	json_ok(Page { has_more, items: responses })
}

/// POST /api/v1/chats
pub async fn create_chat(State(state): State<Arc<JobState>>, user: AuthUser, Json(req): Json<CreateChatRequest>) -> Result<Response, AppError> {
	if let Some(workspace_id) = req.workspace_id
		&& !Chat::verify_workspace_belongs_to_user(&state.db, &workspace_id, &user.id).await? {
			return Err(ErrorCode::NotFound.into());
		}

	let chat = Chat::create(&state.db, &user.id, req.workspace_id.as_ref(), req.title.as_deref()).await?;
	let response = build_chat_response(&state.db, &chat).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// GET /api/v1/chats/:id
pub async fn get_chat(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let chat = Chat::find_by_id_and_user(&state.db, &id, &user.id).await?.ok_or(ErrorCode::NotFound)?;
	let messages = Message::list_active_by_chat(&state.db, &id).await?;
	let chat_response = build_chat_response(&state.db, &chat).await?;
	let messages = ChatMessageResponse::list_for(&state.db, &id, messages).await?;
	json_ok(ChatWithMessagesResponse { chat: chat_response, messages })
}

/// PATCH /api/v1/chats/:id
pub async fn update_chat(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateChatRequest>) -> Result<Response, AppError> {
	if let Some(Some(workspace_id)) = &req.workspace_id
		&& !Chat::verify_workspace_belongs_to_user(&state.db, workspace_id, &user.id).await? {
			return Err(ErrorCode::NotFound.into());
		}

	let chat = Chat::update(
		&state.db,
		&id,
		&user.id,
		req.title.as_deref(),
		req.workspace_id.as_ref().map(|w| w.as_ref()),
		req.is_pinned,
		req.is_archived,
	)
	.await?
	.ok_or(ErrorCode::NotFound)?;
	json_ok(build_chat_response(&state.db, &chat).await?)
}

/// DELETE /api/v1/chats/:id
pub async fn delete_chat(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	if Chat::delete(&state.db, &id, &user.id).await? {
		Ok((StatusCode::NO_CONTENT, "").into_response())
	} else {
		Err(ErrorCode::NotFound.into())
	}
}
