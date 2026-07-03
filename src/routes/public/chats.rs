use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{Chat, ChatListParams, ChatMessageResponse, ChatResponse, ChatWithMessagesResponse, CreateChatRequest, Message, UpdateChatRequest};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;
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
pub async fn list_chats(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ChatListParams>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let limit = params.limit.unwrap_or(50).min(100) as i64;
	let offset = params.offset.unwrap_or(0) as i64;

	let chats = Chat::list_by_user(
		&state.db,
		&user.id,
		params.workspace_id.as_ref(),
		params.include_archived,
		limit,
		offset,
	)
	.await;

	match chats {
		Ok(chats) => {
			let chat_ids: Vec<Uuid> = chats.iter().map(|c| c.id).collect();
			let stats = match Chat::message_stats_batch(&state.db, &chat_ids).await {
				Ok(s) => s,
				Err(e) => {
					eprintln!("[CHATS] Failed to load message stats: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};
			let responses = chats
				.iter()
				.map(|chat| {
					let (count, last) = stats.get(&chat.id).copied().unwrap_or((0, None));
					chat_response_with_stats(chat, count, last)
				})
				.collect::<Vec<_>>();
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[CHATS] Failed to list chats: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats
pub async fn create_chat(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateChatRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if let Some(workspace_id) = req.workspace_id {
		match Chat::verify_workspace_belongs_to_user(&state.db, &workspace_id, &user.id).await {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to validate workspace: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
			_ => {}
		}
	}

	match Chat::create(&state.db, &user.id, req.workspace_id.as_ref(), req.title.as_deref()).await {
		Ok(chat) => match build_chat_response(&state.db, &chat).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to build chat response: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		},
		Err(e) => {
			eprintln!("[CHATS] Failed to create chat: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// GET /api/v1/chats/:id
pub async fn get_chat(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let chat = match Chat::find_by_id_and_user(&state.db, &id, &user.id).await {
		Ok(Some(c)) => c,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to get chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let messages = match Message::list_active_by_chat(&state.db, &id).await {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[CHATS] Failed to get messages: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let chat_response = match build_chat_response(&state.db, &chat).await {
		Ok(r) => r,
		Err(e) => {
			eprintln!("[CHATS] Failed to build chat response: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();

	let sibling_counts = match Message::sibling_counts_for_chat(&state.db, &id).await {
		Ok(c) => c,
		Err(e) => {
			eprintln!("[CHATS] Failed to get sibling counts: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let mut tool_executions = match Message::tool_executions_for_messages(&state.db, &message_ids).await {
		Ok(t) => t,
		Err(e) => {
			eprintln!("[CHATS] Failed to get tool executions: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let message_responses: Vec<ChatMessageResponse> = messages
		.into_iter()
		.map(|m| {
			let msg_id = m.id;
			let parent_id = m.parent_id;
			let role = m.role.clone();
			let mut response = ChatMessageResponse::from(m);
			if let Some(tools) = tool_executions.remove(&msg_id) {
				if !tools.is_empty() {
					response.tool_calls = Some(tools);
				}
			}
			response.sibling_count = sibling_counts.get(&(parent_id, role)).copied().unwrap_or(1) as i32;
			response
		})
		.collect();

	ResponseBuilder::new(ResponseBody::Json(ChatWithMessagesResponse {
		chat: chat_response,
		messages: message_responses,
	}))
	.build()
}

/// PATCH /api/v1/chats/:id
pub async fn update_chat(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateChatRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if let Some(Some(workspace_id)) = &req.workspace_id {
		match Chat::verify_workspace_belongs_to_user(&state.db, workspace_id, &user.id).await {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to validate workspace: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
			_ => {}
		}
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
	.await;

	match chat {
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to update chat: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
		Ok(Some(chat)) => match build_chat_response(&state.db, &chat).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to build chat response: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		},
	}
}

/// DELETE /api/v1/chats/:id
pub async fn delete_chat(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Chat::delete(&state.db, &id, &user.id).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to delete chat: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
