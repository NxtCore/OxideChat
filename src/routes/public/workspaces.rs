//! Workspace management routes.
//!
//! CRUD operations for user workspaces.

use crate::AppState;
use crate::routes::public::auth::get_current_user;
use crate::types::{CreateWorkspaceRequest, UpdateWorkspaceRequest, Workspace, WorkspaceResponse, WorkspaceWithCount};
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
///
/// List all workspaces for the current user.
pub async fn list_workspaces(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Get workspaces with chat counts
	let workspaces = sqlx::query_as::<_, WorkspaceWithCount>(
		r#"
		SELECT w.*, COALESCE(c.chat_count, 0) as chat_count
		FROM workspaces w
		LEFT JOIN (
			SELECT workspace_id, COUNT(*) as chat_count
			FROM chats
			WHERE is_archived = false
			GROUP BY workspace_id
		) c ON w.id = c.workspace_id
		WHERE w.user_id = $1
		ORDER BY w.sort_order, w.name
		"#,
	)
	.bind(user.id)
	.fetch_all(&state.db)
	.await;

	match workspaces {
		Ok(workspaces) => {
			let responses: Vec<WorkspaceResponse> = workspaces.into_iter().map(Into::into).collect();
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to list workspaces: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/workspaces
///
/// Create a new workspace.
pub async fn create_workspace(State(state): State<Arc<AppState>>, cookies: Cookies, Json(req): Json<CreateWorkspaceRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// If this is set as default, unset other defaults first
	if req.is_default {
		if let Err(e) = sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1")
			.bind(user.id)
			.execute(&state.db)
			.await
		{
			eprintln!("[WORKSPACES] Failed to unset default: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	let workspace = sqlx::query_as::<_, Workspace>(
		r#"
		INSERT INTO workspaces (user_id, name, icon, color, is_default)
		VALUES ($1, $2, $3, $4, $5)
		RETURNING *
		"#,
	)
	.bind(user.id)
	.bind(&req.name)
	.bind(&req.icon)
	.bind(&req.color)
	.bind(req.is_default)
	.fetch_one(&state.db)
	.await;

	match workspace {
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
///
/// Get a specific workspace.
pub async fn get_workspace(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let workspace = sqlx::query_as::<_, WorkspaceWithCount>(
		r#"
		SELECT w.*, COALESCE(c.chat_count, 0) as chat_count
		FROM workspaces w
		LEFT JOIN (
			SELECT workspace_id, COUNT(*) as chat_count
			FROM chats
			WHERE is_archived = false
			GROUP BY workspace_id
		) c ON w.id = c.workspace_id
		WHERE w.id = $1 AND w.user_id = $2
		"#,
	)
	.bind(id)
	.bind(user.id)
	.fetch_optional(&state.db)
	.await;

	match workspace {
		Ok(Some(ws)) => {
			let response: WorkspaceResponse = ws.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to get workspace: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/workspaces/:id
///
/// Update a workspace.
pub async fn update_workspace(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateWorkspaceRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	// Get existing workspace
	let existing = sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1 AND user_id = $2")
		.bind(id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let existing = match existing {
		Ok(Some(ws)) => ws,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to fetch workspace for update: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// If setting as default, unset other defaults first
	if req.is_default == Some(true) {
		if let Err(e) = sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1 AND id != $2")
			.bind(user.id)
			.bind(id)
			.execute(&state.db)
			.await
		{
			eprintln!("[WORKSPACES] Failed to unset default: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	// Apply updates
	let name = req.name.unwrap_or(existing.name);
	let icon = req.icon.or(existing.icon);
	let color = req.color.or(existing.color);
	let sort_order = req.sort_order.unwrap_or(existing.sort_order);
	let is_default = req.is_default.unwrap_or(existing.is_default);

	let workspace = sqlx::query_as::<_, Workspace>(
		r#"
		UPDATE workspaces
		SET name = $3, icon = $4, color = $5, sort_order = $6, is_default = $7, updated_at = NOW()
		WHERE id = $1 AND user_id = $2
		RETURNING *
		"#,
	)
	.bind(id)
	.bind(user.id)
	.bind(&name)
	.bind(&icon)
	.bind(&color)
	.bind(sort_order)
	.bind(is_default)
	.fetch_one(&state.db)
	.await;

	match workspace {
		Ok(ws) => {
			let response = WorkspaceResponse::from_workspace(ws, 0);
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[WORKSPACES] Failed to update workspace: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
	}
}

/// DELETE /api/v1/workspaces/:id
///
/// Delete a workspace (chats will have workspace_id set to NULL).
pub async fn delete_workspace(State(state): State<Arc<AppState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let result = sqlx::query("DELETE FROM workspaces WHERE id = $1 AND user_id = $2")
		.bind(id)
		.bind(user.id)
		.execute(&state.db)
		.await;

	match result {
		Ok(res) if res.rows_affected() > 0 => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(_) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[WORKSPACES] Failed to delete workspace: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
