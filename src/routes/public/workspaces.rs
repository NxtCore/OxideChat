use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::{CreateWorkspaceRequest, UpdateWorkspaceRequest, Workspace, WorkspaceResponse};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
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

	if req.is_default {
		if let Err(e) = Workspace::clear_default_for_user(&state.db, &user.id, None).await {
			eprintln!("[WORKSPACES] Failed to unset default: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	match Workspace::create(&state.db, &user.id, &req.name, req.icon.as_deref(), req.color.as_deref(), req.is_default).await {
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

	if req.is_default == Some(true) {
		if let Err(e) = Workspace::clear_default_for_user(&state.db, &user.id, Some(&id)).await {
			eprintln!("[WORKSPACES] Failed to unset default: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	let updated = Workspace::update(
		&state.db,
		&id,
		&user.id,
		req.name.as_deref(),
		req.icon.as_deref(),
		req.color.as_deref(),
		req.sort_order,
		req.is_default,
	)
	.await;

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
pub async fn delete_workspace(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Workspace::delete(&state.db, &id, &user.id).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to delete workspace: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
