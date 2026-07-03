use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{Chat, CreateWorkspaceRequest, DeleteWorkspaceParams, UpdateWorkspaceRequest, Workspace, WorkspaceDeleteAction, WorkspaceResponse};
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

/// GET /api/v1/workspaces
pub async fn list_workspaces(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Workspace::list_with_counts(&state.db, &user.id).await {
		Ok(workspaces) => ResponseBuilder::new(ResponseBody::Json(workspaces)).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to list workspaces: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/workspaces
pub async fn create_workspace(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateWorkspaceRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Workspace::create_from_request(&state.db, &user.id, &req).await {
		Ok(ws) => {
			let response = WorkspaceResponse::from_workspace(ws, 0);
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[WORKSPACES] Failed to create workspace: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
	}
}

/// GET /api/v1/workspaces/:id
pub async fn get_workspace(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Workspace::find_with_count(&state.db, &id, &user.id).await {
		Ok(Some(ws)) => ResponseBuilder::new(ResponseBody::Json(ws)).build(),
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to get workspace: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/workspaces/:id
pub async fn update_workspace(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateWorkspaceRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let updated = Workspace::update_from_request(&state.db, &id, &user.id, &req).await;

	match updated {
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[WORKSPACES] Failed to update workspace: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
		Ok(Some(_)) => match Workspace::find_with_count(&state.db, &id, &user.id).await {
			Ok(Some(ws)) => ResponseBuilder::new(ResponseBody::Json(ws)).build(),
			Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[WORKSPACES] Failed to fetch updated workspace: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		},
	}
}

/// DELETE /api/v1/workspaces/:id
pub async fn delete_workspace(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(id): Path<Uuid>,
	Query(params): Query<DeleteWorkspaceParams>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let workspace = match Workspace::find_by_id_and_user(&state.db, &id, &user.id).await {
		Ok(Some(ws)) => ws,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to load workspace: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	if workspace.is_default {
		return ErrorBuilder::new(ErrorCode::DefaultWorkspaceDelete).build();
	}

	if let WorkspaceDeleteAction::Move = params.action {
		let Some(target) = params.target_workspace_id else {
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
		};
		if target == id {
			return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
		}
		match Chat::verify_workspace_belongs_to_user(&state.db, &target, &user.id).await {
			Ok(true) => {}
			Ok(false) => return ErrorBuilder::new(ErrorCode::ValidationFailed).build(),
			Err(e) => {
				eprintln!("[WORKSPACES] Failed to validate target workspace: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
		}
	}

	match Workspace::delete_with_chat_disposition(&state.db, &id, &user.id, params.action, params.target_workspace_id.as_ref()).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to delete workspace: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
