//! Admin routes for roles and the permissions they grant.

use crate::types::consts::{ADMIN_ROLES_EDIT, ADMIN_ROLES_VIEW};
use crate::types::{AppError, AuthUser, JobState, Role, RoleActor, SaveRoleRequest, json_ok};
use crate::utils::response::{ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/roles
pub async fn list_roles(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ROLES_VIEW).await?;
	json_ok(Role::list_detailed(&state.db).await?)
}

/// GET /api/v1/admin/permissions
pub async fn list_permissions(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ROLES_VIEW).await?;
	json_ok(Role::list_permissions(&state.db).await?)
}

/// POST /api/v1/admin/roles
pub async fn create_role(State(state): State<Arc<JobState>>, auth: AuthUser, Json(request): Json<SaveRoleRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ROLES_EDIT).await?;
	let permissions = auth.permissions(&state.db).await?;
	let actor = RoleActor {
		id: &auth.id,
		permissions: &permissions,
	};
	let role = Role::create(&state.db, &actor, &request).await?;
	Ok(ResponseBuilder::new(ResponseBody::Json(role)).status(StatusCode::CREATED).build())
}

/// PUT /api/v1/admin/roles/{id}
pub async fn update_role(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SaveRoleRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ROLES_EDIT).await?;
	let permissions = auth.permissions(&state.db).await?;
	let actor = RoleActor {
		id: &auth.id,
		permissions: &permissions,
	};
	json_ok(Role::update(&state.db, &actor, &id, &request).await?)
}

/// DELETE /api/v1/admin/roles/{id}
pub async fn delete_role(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ROLES_EDIT).await?;
	let permissions = auth.permissions(&state.db).await?;
	let actor = RoleActor {
		id: &auth.id,
		permissions: &permissions,
	};
	Role::delete(&state.db, &actor, &id).await?;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}
