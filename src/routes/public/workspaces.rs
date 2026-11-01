use crate::types::{AppError, AuthUser, Chat, CreateWorkspaceRequest, DeleteWorkspaceParams, JobState, UpdateWorkspaceRequest, Workspace, WorkspaceDeleteAction, WorkspaceResponse, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/workspaces
pub async fn list_workspaces(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(Workspace::list_with_counts(&state.db, &user.id).await?)
}

/// POST /api/v1/workspaces
pub async fn create_workspace(State(state): State<Arc<JobState>>, user: AuthUser, Json(req): Json<CreateWorkspaceRequest>) -> Result<Response, AppError> {
	let ws = Workspace::create_from_request(&state.db, &user.id, &req).await.map_err(|e| AppError::unique_or(e, ErrorCode::AlreadyExists))?;
	Ok(ResponseBuilder::new(ResponseBody::Json(WorkspaceResponse::from_workspace(ws, 0))).status(StatusCode::CREATED).build())
}

/// GET /api/v1/workspaces/:id
pub async fn get_workspace(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	json_ok(Workspace::find_with_count(&state.db, &id, &user.id).await?.ok_or(ErrorCode::NotFound)?)
}

/// PATCH /api/v1/workspaces/:id
pub async fn update_workspace(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateWorkspaceRequest>) -> Result<Response, AppError> {
	Workspace::update_from_request(&state.db, &id, &user.id, &req)
		.await
		.map_err(|e| AppError::unique_or(e, ErrorCode::AlreadyExists))?
		.ok_or(ErrorCode::NotFound)?;
	json_ok(Workspace::find_with_count(&state.db, &id, &user.id).await?.ok_or(ErrorCode::NotFound)?)
}

/// DELETE /api/v1/workspaces/:id
pub async fn delete_workspace(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Query(params): Query<DeleteWorkspaceParams>) -> Result<Response, AppError> {
	let workspace = Workspace::find_by_id_and_user(&state.db, &id, &user.id).await?.ok_or(ErrorCode::NotFound)?;

	if workspace.is_default {
		return Err(ErrorCode::DefaultWorkspaceDelete.into());
	}

	if let WorkspaceDeleteAction::Move = params.action {
		let target = params.target_workspace_id.ok_or(ErrorCode::ValidationFailed)?;
		if target == id || !Chat::verify_workspace_belongs_to_user(&state.db, &target, &user.id).await? {
			return Err(ErrorCode::ValidationFailed.into());
		}
	}

	if Workspace::delete_with_chat_disposition(&state.db, &id, &user.id, params.action, params.target_workspace_id.as_ref()).await? {
		Ok((StatusCode::NO_CONTENT, "").into_response())
	} else {
		Err(ErrorCode::NotFound.into())
	}
}
