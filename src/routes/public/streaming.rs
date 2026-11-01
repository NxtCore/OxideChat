//! Streaming routes for AI responses.
//!
//! Server-Sent Events (SSE) endpoint for streaming AI chat completions.

use crate::types::chat::runner::{prepare_turn, stream_turn};
use crate::types::{AppError, AuthUser, ClientToolResultRequest, JobState, StreamRequest, json_ok};
use crate::utils::response::ErrorCode;
use axum::{
	Json,
	extract::{Path, State},
	response::{
		IntoResponse, Response,
		sse::{KeepAlive, Sse},
	},
};
use std::{sync::Arc, time::Instant};
use uuid::Uuid;

pub async fn stream_completion(State(state): State<Arc<JobState>>, user: AuthUser, Path(chat_id): Path<Uuid>, Json(req): Json<StreamRequest>) -> Response {
	match prepare_turn(&state, &user, chat_id, &req).await {
		Ok(turn) => Sse::new(stream_turn(state, turn, Instant::now())).keep_alive(KeepAlive::default()).into_response(),
		Err(error) => error.into_response(),
	}
}

/// POST `/api/v1/chats/{chat_id}/stream/tool-result`
///
/// Called by the browser after it has executed a client-side MCP tool call.
/// The result is forwarded to the waiting streaming loop via the oneshot channel
/// that was registered when the `ClientToolCall` SSE event was emitted.
pub async fn submit_client_tool_result(
	State(state): State<Arc<JobState>>,
	user: AuthUser,
	Path(_chat_id): Path<Uuid>,
	Json(req): Json<ClientToolResultRequest>,
) -> Result<Response, AppError> {
	if state.client_tool_pending.resolve(&req.call_id, user.id, req.result).await {
		json_ok(serde_json::json!({"ok": true}))
	} else {
		Err(ErrorCode::NotFound.into())
	}
}
