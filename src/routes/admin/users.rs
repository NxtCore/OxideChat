//! Admin user management routes.
//!
//! CRUD operations for user accounts, role assignment, and password resets.

use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::consts::{ADMIN_USERS_EDIT, ADMIN_USERS_VIEW};
use crate::types::{AdminResetPasswordRequest, AppError, AuthUser, CreateAdminUserRequest, JobState, ListUsersQuery, PageQuery, SetUserRolesRequest, SetUserTeamsRequest, UpdateUserRequest, User, json_ok};
use crate::utils::auth::{hash_password, validate_email, validate_password, validate_username};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
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

/// Create a new user.
pub async fn create_user(State(state): State<Arc<JobState>>, actor: AuthUser, Json(req): Json<CreateAdminUserRequest>) -> Result<Response, AppError> {
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

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

	let new_user = User::create(&state.db, &req.email, &req.username, &password_hash).await.map_err(AppError::database)?;
	new_user.initialize_defaults(&state.db).await.map_err(AppError::database)?;

	for role_name in &req.roles {
		if !crate::types::Role::exists(&state.db, role_name).await.map_err(AppError::database)? {
			return Err(ErrorCode::NotFound.into());
		}
	}

	new_user.set_roles(&state.db, &req.roles).await.map_err(AppError::database)?;

	if let Some(team_ids) = &req.team_ids {
		new_user.set_teams(&state.db, team_ids).await.map_err(AppError::database)?;
	}

	AuditLog::log(&state.db, LogEvent::UserRegistered, Some(actor.id), Some(EntityType::User), Some(new_user.id));

	let response = new_user.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// Update a user's email or username.
pub async fn update_user(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateUserRequest>) -> Result<Response, AppError> {
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

	let mut target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

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
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

	if actor.id == id {
		return Err(ErrorCode::Forbidden.into());
	}

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

	AuditLog::log(&state.db, LogEvent::UserDeleted, Some(actor.id), Some(EntityType::User), Some(id));

	if !target.delete(&state.db).await.map_err(AppError::database)? {
		return Err(ErrorCode::UserNotFound.into());
	}

	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// Replace the full role set for a user. Admins cannot change their own roles.
pub async fn set_user_roles(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<SetUserRolesRequest>) -> Result<Response, AppError> {
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

	if actor.id == id {
		return Err(ErrorCode::Forbidden.into());
	}

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

	for role_name in &req.roles {
		if !crate::types::Role::exists(&state.db, role_name).await.map_err(AppError::database)? {
			return Err(ErrorCode::NotFound.into());
		}
	}

	target.set_roles(&state.db, &req.roles).await.map_err(AppError::database)?;

	AuditLog::log(&state.db, LogEvent::RoleAssigned, Some(actor.id), Some(EntityType::User), Some(id));

	let response = target.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

pub async fn set_user_teams(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<SetUserTeamsRequest>) -> Result<Response, AppError> {
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

	target.set_teams(&state.db, &req.team_ids).await.map_err(AppError::database)?;

	AuditLog::log(&state.db, LogEvent::UserUpdated, Some(actor.id), Some(EntityType::User), Some(id));

	let response = target.to_response(&state.db).await.map_err(AppError::database)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// Reset a user's password.
pub async fn reset_password(State(state): State<Arc<JobState>>, actor: AuthUser, Path(id): Path<Uuid>, Json(req): Json<AdminResetPasswordRequest>) -> Result<Response, AppError> {
	actor.require(&state.db, ADMIN_USERS_EDIT).await?;

	let target = User::find_by_id(&state.db, &id).await.map_err(AppError::database)?.ok_or(ErrorCode::UserNotFound)?;

	if target.auth_method != "local" {
		return Err(ErrorCode::ExternalAuthRequired.into());
	}

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
