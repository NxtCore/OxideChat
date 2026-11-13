//! Admin user management routes.
//!
//! CRUD operations for user accounts, role assignment, and password resets.

use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::consts::{ADMIN_USERS_EDIT, ADMIN_USERS_VIEW};
use crate::types::{
	AdminResetPasswordRequest, AppError, AuthUser, CreateAdminUserRequest, JobState, ListUsersQuery, PageQuery, ReplaceAssignmentsRequest, Role, RoleError,
	UpdateUserRequest, User, json_ok,
};
use crate::utils::auth::{SESSION_COOKIE_NAME, hash_password, validate_email, validate_password, validate_username};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

/// List users with optional pagination, search, and role filter.
pub async fn list_users(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ListUsersQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_USERS_VIEW).await?;

	let response = User::list_paginated_light(&state.db, &page, params.search.as_deref(), params.role.as_deref(), params.team_id.as_ref()).await.map_err(AppError::database)?;
	json_ok(response)
}

/// Get a single user by ID.
pub async fn get_user(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_USERS_VIEW).await?;

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;
	let response = target.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// GET /api/v1/admin/users/{id}/overview
///
/// Everything that applies to one user: profile, sessions, roles, effective permissions, teams and, for the admin areas the
/// caller may view, model access, presets, budgets and API keys.
pub async fn get_user_overview(State(state): State<Arc<JobState>>, auth: AuthUser, cookies: Cookies, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let permissions = auth.require_permissions(&state.db, ADMIN_USERS_VIEW).await?;
	let target = User::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::UserNotFound)?;
	let actor = auth.role_actor(&permissions);
	let session = cookies.get(SESSION_COOKIE_NAME).and_then(|cookie| Uuid::parse_str(cookie.value()).ok());
	json_ok(target.overview(&state.db, &actor, session.as_ref()).await?)
}

/// GET /api/v1/admin/users/{id}/assignments
pub async fn get_user_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let permissions = auth.require_permissions(&state.db, ADMIN_USERS_VIEW).await?;
	let target = User::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::UserNotFound)?;
	let actor = auth.role_actor(&permissions);
	json_ok(target.assignments(&state.db, &actor).await?)
}

/// PUT /api/v1/admin/users/{id}/assignments
///
/// Replaces the roles and teams of a user.
pub async fn replace_user_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<ReplaceAssignmentsRequest>) -> Result<Response, AppError> {
	let permissions = auth.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;
	if !req.is_valid_for(&User::ASSIGNMENT_KINDS) {
		return Err(ErrorCode::ValidationFailed.into());
	}
	let target = User::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::UserNotFound)?;
	let actor = auth.role_actor(&permissions);
	target.replace_assignments(&state.db, &actor, &req).await.map_err(|error| match error {
		RoleError::Database(error) => AppError::foreign_key_or(error, ErrorCode::ValidationFailed),
		other => other.into(),
	})?;
	json_ok(target.assignments(&state.db, &actor).await?)
}

/// DELETE `/api/v1/admin/users/{id}/sessions/{session_id}`
///
/// Signs a user out of one of their sessions.
pub async fn revoke_user_session(State(state): State<Arc<JobState>>, auth: AuthUser, Path((id, session_id)): Path<(Uuid, String)>) -> Result<Response, AppError> {
	let permissions = auth.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;
	let target = User::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::UserNotFound)?;
	let actor = auth.role_actor(&permissions);
	actor.check_outranks(&state.db, &target).await?;
	if !User::revoke_session(&state.db, &id, &session_id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	AuditLog::log(&state.db, LogEvent::SessionRevoked, Some(auth.id), Some(EntityType::User), Some(id));
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// Create a new user.
pub async fn create_user(State(state): State<Arc<JobState>>, actor: AuthUser, Json(req): Json<CreateAdminUserRequest>) -> Result<Response, AppError> {
	let permissions = actor.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;

	if !validate_email(&req.email) {
		return Err(ErrorCode::InvalidEmail.into());
	}
	validate_username(&req.username)?;
	validate_password(&req.password)?;

	if User::email_exists(&state.db, &req.email, None).await.map_err(AppError::database)? {
		return Err(ErrorCode::EmailTaken.into());
	}
	if User::username_exists(&state.db, &req.username, None).await.map_err(AppError::database)? {
		return Err(ErrorCode::UsernameTaken.into());
	}

	let password_hash = hash_password(&req.password).map_err(|error| {
		tracing::error!("Failed to hash password: {error}");
		ErrorCode::InternalError
	})?;

	for role_name in &req.roles {
		if !Role::exists(&state.db, role_name).await.map_err(AppError::database)? {
			return Err(ErrorCode::NotFound.into());
		}
	}
	let role_actor = actor.role_actor(&permissions);
	role_actor.check_role_change(&state.db, &[], &req.roles).await?;

	let new_user = User::create_with_assignments(&state.db, &req, &password_hash)
		.await
		.map_err(|error| AppError::foreign_key_or(error, ErrorCode::ValidationFailed))?;

	AuditLog::log(&state.db, LogEvent::UserRegistered, Some(actor.id), Some(EntityType::User), Some(new_user.id));

	let response = new_user.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// Update a user's email or username.
pub async fn update_user(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateUserRequest>) -> Result<Response, AppError> {
	let permissions = actor.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;

	let mut target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;
	let role_actor = actor.role_actor(&permissions);
	role_actor.check_outranks(&state.db, &target).await?;

	if let Some(email) = &req.email {
		if !validate_email(email) {
			return Err(ErrorCode::InvalidEmail.into());
		}
		if email != &target.email && User::email_exists(&state.db, email, Some(&id)).await.map_err(AppError::database)? {
			return Err(ErrorCode::EmailTaken.into());
		}
	}

	if let Some(username) = &req.username {
		validate_username(username)?;
		if username != &target.username && User::username_exists(&state.db, username, Some(&id)).await.map_err(AppError::database)? {
			return Err(ErrorCode::UsernameTaken.into());
		}
	}

	target.update(&state.db, req.email.as_deref(), req.username.as_deref()).await.map_err(AppError::database)?;

	AuditLog::log(&state.db, LogEvent::UserUpdated, Some(actor.id), Some(EntityType::User), Some(id));

	let response = target.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// Delete a user. Admins cannot delete their own account.
pub async fn delete_user(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let permissions = actor.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;

	if actor.id == id {
		return Err(ErrorCode::Forbidden.into());
	}

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;
	let role_actor = actor.role_actor(&permissions);
	role_actor.check_outranks(&state.db, &target).await?;

	if !target.delete(&state.db).await? {
		return Err(ErrorCode::UserNotFound.into());
	}

	AuditLog::log(&state.db, LogEvent::UserDeleted, Some(actor.id), Some(EntityType::User), Some(id));

	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// Reset a user's password.
pub async fn reset_password(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<AdminResetPasswordRequest>) -> Result<Response, AppError> {
	let permissions = actor.require_permissions(&state.db, ADMIN_USERS_EDIT).await?;

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

	if target.auth_method != "local" {
		return Err(ErrorCode::ExternalAuthRequired.into());
	}
	let role_actor = actor.role_actor(&permissions);
	role_actor.check_outranks(&state.db, &target).await?;

	validate_password(&req.password)?;

	let password_hash = hash_password(&req.password).map_err(|error| {
		tracing::error!("Failed to hash password: {error}");
		ErrorCode::InternalError
	})?;

	target.set_password(&state.db, &password_hash).await.map_err(AppError::database)?;

	AuditLog::log(&state.db, LogEvent::PasswordChanged, Some(actor.id), Some(EntityType::User), Some(id));

	Ok(ResponseBuilder::new(ResponseBody::Json(crate::types::MessageResponse {
		message: "Password updated successfully".to_string(),
	}))
	.build())
}
