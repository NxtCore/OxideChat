//! Chat management routes.
//!
//! CRUD operations for user chats.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::types::{Chat, ChatListParams, ChatMessageResponse, ChatResponse, ChatWithMessagesResponse, CreateChatRequest, Message, UpdateChatRequest};
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

/// Helper to build ChatResponse with message count and last message time
async fn build_chat_response(pool: &sqlx::PgPool, chat: Chat) -> Result<ChatResponse, sqlx::Error> {
	let stats: (i64, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT COUNT(*), MAX(created_at) FROM messages WHERE chat_id = $1")
		.bind(chat.id)
		.fetch_one(pool)
		.await?;

	Ok(ChatResponse {
		id: chat.id,
		workspace_id: chat.workspace_id,
		title: chat.title,
		is_pinned: chat.is_pinned,
		is_archived: chat.is_archived,
		message_count: stats.0,
		last_message_at: stats.1,
		created_at: chat.created_at,
		updated_at: chat.updated_at,
	})
}

/// GET /api/v1/chats
///
/// List chats with optional filters.
pub async fn list_chats(State(state): State<Arc<AppState>>, cookies: Cookies, Query(params): Query<ChatListParams>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let limit = params.limit.unwrap_or(50).min(100);
	let offset = params.offset.unwrap_or(0);

	// Build query based on filters
	let chats = if let Some(workspace_id) = params.workspace_id {
		if params.include_archived {
			sqlx::query_as::<_, Chat>(
				r#"
				SELECT * FROM chats
				WHERE user_id = $1 AND workspace_id = $2
				ORDER BY is_pinned DESC, updated_at DESC
				LIMIT $3 OFFSET $4
				"#,
			)
			.bind(user.id)
			.bind(workspace_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(&state.db)
			.await
		} else {
			sqlx::query_as::<_, Chat>(
				r#"
				SELECT * FROM chats
				WHERE user_id = $1 AND workspace_id = $2 AND is_archived = false
				ORDER BY is_pinned DESC, updated_at DESC
				LIMIT $3 OFFSET $4
				"#,
			)
			.bind(user.id)
			.bind(workspace_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(&state.db)
			.await
		}
	} else if params.include_archived {
		sqlx::query_as::<_, Chat>(
			r#"
			SELECT * FROM chats
			WHERE user_id = $1
			ORDER BY is_pinned DESC, updated_at DESC
			LIMIT $2 OFFSET $3
			"#,
		)
		.bind(user.id)
		.bind(limit)
		.bind(offset)
		.fetch_all(&state.db)
		.await
	} else {
		sqlx::query_as::<_, Chat>(
			r#"
			SELECT * FROM chats
			WHERE user_id = $1 AND is_archived = false
			ORDER BY is_pinned DESC, updated_at DESC
			LIMIT $2 OFFSET $3
			"#,
		)
		.bind(user.id)
		.bind(limit)
		.bind(offset)
		.fetch_all(&state.db)
		.await
	};

	match chats {
		Ok(chats) => {
			let mut responses = Vec::with_capacity(chats.len());
			for chat in chats {
				match build_chat_response(&state.db, chat).await {
					Ok(response) => responses.push(response),
					Err(e) => {
						eprintln!("[CHATS] Failed to build chat response: {e}");
						return ErrorBuilder::new(ErrorCode::InternalError).build();
					}
				}
			}
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[CHATS] Failed to list chats: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/chats
///
/// Create a new chat.
pub async fn create_chat(State(state): State<Arc<AppState>>, cookies: Cookies, Json(req): Json<CreateChatRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Validate workspace belongs to user if provided
	if let Some(workspace_id) = req.workspace_id {
		let workspace_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2)")
			.bind(workspace_id)
			.bind(user.id)
			.fetch_one(&state.db)
			.await;

		match workspace_exists {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to validate workspace: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
			_ => {}
		}
	}

	let chat = sqlx::query_as::<_, Chat>(
		r#"
		INSERT INTO chats (user_id, workspace_id, title)
		VALUES ($1, $2, $3)
		RETURNING *
		"#,
	)
	.bind(user.id)
	.bind(req.workspace_id)
	.bind(&req.title)
	.fetch_one(&state.db)
	.await;

	match chat {
		Ok(chat) => match build_chat_response(&state.db, chat).await {
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
///
/// Get a chat with its messages.
pub async fn get_chat(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let chat = sqlx::query_as::<_, Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
		.bind(id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let chat = match chat {
		Ok(Some(chat)) => chat,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to get chat: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Get messages
	let messages = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 ORDER BY created_at ASC")
		.bind(id)
		.fetch_all(&state.db)
		.await;

	match messages {
		Ok(messages) => {
			let chat_response = match build_chat_response(&state.db, chat).await {
				Ok(r) => r,
				Err(e) => {
					eprintln!("[CHATS] Failed to build chat response: {e}");
					return ErrorBuilder::new(ErrorCode::InternalError).build();
				}
			};
			let message_responses: Vec<ChatMessageResponse> = messages.into_iter().map(Into::into).collect();
			let response = ChatWithMessagesResponse {
				chat: chat_response,
				messages: message_responses,
			};
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[CHATS] Failed to get messages: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/chats/:id
///
/// Update a chat (rename, pin, archive, move workspace).
pub async fn update_chat(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateChatRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Get existing chat
	let existing = sqlx::query_as::<_, Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
		.bind(id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let existing = match existing {
		Ok(Some(chat)) => chat,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to fetch chat for update: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Validate new workspace if provided
	if let Some(workspace_id) = req.workspace_id {
		let workspace_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2)")
			.bind(workspace_id)
			.bind(user.id)
			.fetch_one(&state.db)
			.await;

		match workspace_exists {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to validate workspace: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
			_ => {}
		}
	}

	// Apply updates
	let title = req.title.or(existing.title);
	let workspace_id = req.workspace_id.or(existing.workspace_id);
	let is_pinned = req.is_pinned.unwrap_or(existing.is_pinned);
	let is_archived = req.is_archived.unwrap_or(existing.is_archived);

	let chat = sqlx::query_as::<_, Chat>(
		r#"
		UPDATE chats
		SET title = $3, workspace_id = $4, is_pinned = $5, is_archived = $6, updated_at = NOW()
		WHERE id = $1 AND user_id = $2
		RETURNING *
		"#,
	)
	.bind(id)
	.bind(user.id)
	.bind(&title)
	.bind(workspace_id)
	.bind(is_pinned)
	.bind(is_archived)
	.fetch_one(&state.db)
	.await;

	match chat {
		Ok(chat) => match build_chat_response(&state.db, chat).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[CHATS] Failed to build chat response: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		},
		Err(e) => {
			eprintln!("[CHATS] Failed to update chat: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// DELETE /api/v1/chats/:id
///
/// Delete a chat and all its messages.
pub async fn delete_chat(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let result = sqlx::query("DELETE FROM chats WHERE id = $1 AND user_id = $2")
		.bind(id)
		.bind(user.id)
		.execute(&state.db)
		.await;

	match result {
		Ok(res) if res.rows_affected() > 0 => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(_) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[CHATS] Failed to delete chat: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
