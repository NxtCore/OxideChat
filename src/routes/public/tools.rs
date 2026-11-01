use crate::types::tools::*;
use crate::types::{AppError, AuthUser, JobState, json_ok};
use axum::{extract::State, response::Response};
use std::sync::Arc;

pub async fn list_tools(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(Tool::list_responses(&state.db, Some(&user.id)).await?)
}
