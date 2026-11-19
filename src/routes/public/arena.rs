//! Arena rounds of a chat: look at a round and pick its winning answer.

use crate::types::{AppError, ArenaPickRequest, ArenaRound, AuthUser, JobState, json_ok};
use crate::utils::response::ErrorCode;
use axum::Json;
use axum::extract::{Path, State};
use axum::response::Response;
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/chats/{chat_id}/arena-rounds/{round_id}
///
/// The round's answers, blind until it is decided and with their models afterwards.
pub async fn get_round(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, round_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	let round = ArenaRound::find_for_user(&state.db, &user.id, &chat_id, &round_id).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(round.to_response(&state.db).await?)
}

/// POST /api/v1/chats/{chat_id}/arena-rounds/{round_id}/pick
///
/// Picks the winning answer, which the chat continues with, and reveals every answer's model.
pub async fn pick(State(state): State<Arc<JobState>>, user: AuthUser, Path((chat_id, round_id)): Path<(Uuid, Uuid)>, Json(request): Json<ArenaPickRequest>) -> Result<Response, AppError> {
	let round = ArenaRound::pick(&state.db, &user.id, &chat_id, &round_id, &request.message_id).await?;
	json_ok(round.to_response(&state.db).await?)
}
