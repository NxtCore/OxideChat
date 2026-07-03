//! Admin user management routes.
//!
//! CRUD operations for user accounts, role assignment, and password resets.

use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::routes::public::auth::get_current_user;
use crate::types::consts::{ADMIN_USERS_EDIT, ADMIN_USERS_VIEW};
use crate::types::{AdminResetPasswordRequest, CreateAdminUserRequest, JobState, ListUsersQuery, SetUserRolesRequest, SetUserTeamsRequest, UpdateUserRequest, User};
use crate::utils::auth::{hash_password, validate_email, validate_password, validate_username};
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

const DEFAULT_PER_PAGE: i64 = 20;
const MAX_PER_PAGE: i64 = 100;

/// List users with optional pagination, search, and role filter.
pub async fn list_users(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ListUsersQuery>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_USERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let page = params.page.unwrap_or(1).max(1);
	let per_page = params.per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE);
	let search = params.search.as_deref();
	let role = params.role.as_deref();

	match User::list_paginated_light(&state.db, page, per_page, search, role, params.team_id.as_ref()).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to list users: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Get a single user by ID.
pub async fn get_user(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_USERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match User::find_by_id(&state.db, &id).await {
		Ok(Some(target)) => match target.to_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[USERS] Failed to build user response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Ok(None) => ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Create a new user.
pub async fn create_user(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateAdminUserRequest>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if !validate_email(&req.email) {
		return ErrorBuilder::new(ErrorCode::InvalidEmail).build();
	}

	if let Err(code) = validate_username(&req.username) {
		return ErrorBuilder::new(code).build();
	}

	if let Err(code) = validate_password(&req.password) {
		return ErrorBuilder::new(code).build();
	}

	match User::email_exists(&state.db, &req.email, None).await {
		Ok(true) => return ErrorBuilder::new(ErrorCode::EmailTaken).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to check email uniqueness: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
		_ => {}
	}

	match User::username_exists(&state.db, &req.username, None).await {
		Ok(true) => return ErrorBuilder::new(ErrorCode::UsernameTaken).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to check username uniqueness: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
		_ => {}
	}

	let password_hash = match hash_password(&req.password) {
		Ok(h) => h,
		Err(e) => {
			eprintln!("[USERS] Failed to hash password: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let new_user = match User::create(&state.db, &req.email, &req.username, &password_hash).await {
		Ok(u) => u,
		Err(e) => {
			eprintln!("[USERS] Failed to insert user: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	if let Err(e) = new_user.initialize_defaults(&state.db).await {
		eprintln!("[USERS] Failed to initialize user defaults: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	for role_name in &req.roles {
		match crate::types::Role::exists(&state.db, role_name).await {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[USERS] Failed to validate role '{role_name}': {e}");
				return ErrorBuilder::new(ErrorCode::DatabaseError).build();
			}
			_ => {}
		}
	}

	if let Err(e) = new_user.set_roles(&state.db, &req.roles).await {
		eprintln!("[USERS] Failed to assign roles: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	if let Some(team_ids) = &req.team_ids {
		if let Err(e) = new_user.set_teams(&state.db, team_ids).await {
			eprintln!("[USERS] Failed to assign teams: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	}

	AuditLog::log(&state.db, LogEvent::UserRegistered, Some(actor.id), Some(EntityType::User), Some(new_user.id));

	match new_user.to_response(&state.db).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to build user response: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Update a user's email or username.
pub async fn update_user(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateUserRequest>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let mut target = match User::find_by_id(&state.db, &id).await {
		Ok(Some(u)) => u,
		Ok(None) => return ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	if let Some(email) = &req.email {
		if !validate_email(email) {
			return ErrorBuilder::new(ErrorCode::InvalidEmail).build();
		}
		if email != &target.email {
			match User::email_exists(&state.db, email, Some(&id)).await {
				Ok(true) => return ErrorBuilder::new(ErrorCode::EmailTaken).build(),
				Err(e) => {
					eprintln!("[USERS] Failed to check email uniqueness: {e}");
					return ErrorBuilder::new(ErrorCode::DatabaseError).build();
				}
				_ => {}
			}
		}
	}

	if let Some(username) = &req.username {
		if let Err(code) = validate_username(username) {
			return ErrorBuilder::new(code).build();
		}
		if username != &target.username {
			match User::username_exists(&state.db, username, Some(&id)).await {
				Ok(true) => return ErrorBuilder::new(ErrorCode::UsernameTaken).build(),
				Err(e) => {
					eprintln!("[USERS] Failed to check username uniqueness: {e}");
					return ErrorBuilder::new(ErrorCode::DatabaseError).build();
				}
				_ => {}
			}
		}
	}

	if let Err(e) = target.update(&state.db, req.email.as_deref(), req.username.as_deref()).await {
		eprintln!("[USERS] Failed to update user: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::UserUpdated, Some(actor.id), Some(EntityType::User), Some(id));

	match target.to_response(&state.db).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to build user response: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Delete a user. Admins cannot delete their own account.
pub async fn delete_user(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if actor.id == id {
		return ErrorBuilder::new(ErrorCode::Forbidden).build();
	}

	let target = match User::find_by_id(&state.db, &id).await {
		Ok(Some(u)) => u,
		Ok(None) => return ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user for deletion: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	AuditLog::log(&state.db, LogEvent::UserDeleted, Some(actor.id), Some(EntityType::User), Some(id));

	match target.delete(&state.db).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to delete user: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Replace the full role set for a user. Admins cannot change their own roles.
pub async fn set_user_roles(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<SetUserRolesRequest>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if actor.id == id {
		return ErrorBuilder::new(ErrorCode::Forbidden).build();
	}

	let target = match User::find_by_id(&state.db, &id).await {
		Ok(Some(u)) => u,
		Ok(None) => return ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user for role update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	for role_name in &req.roles {
		match crate::types::Role::exists(&state.db, role_name).await {
			Ok(false) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[USERS] Failed to validate role '{role_name}': {e}");
				return ErrorBuilder::new(ErrorCode::DatabaseError).build();
			}
			_ => {}
		}
	}

	if let Err(e) = target.set_roles(&state.db, &req.roles).await {
		eprintln!("[USERS] Failed to set roles: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::RoleAssigned, Some(actor.id), Some(EntityType::User), Some(id));

	match target.to_response(&state.db).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to build user response: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn set_user_teams(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<SetUserTeamsRequest>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let target = match User::find_by_id(&state.db, &id).await {
		Ok(Some(u)) => u,
		Ok(None) => return ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user for team update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	if let Err(e) = target.set_teams(&state.db, &req.team_ids).await {
		eprintln!("[USERS] Failed to set teams: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::UserUpdated, Some(actor.id), Some(EntityType::User), Some(id));

	match target.to_response(&state.db).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to build user response: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

/// Reset a user's password.
pub async fn reset_password(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<AdminResetPasswordRequest>) -> impl IntoResponse {
	let actor = match get_current_user(&state.db, &cookies).await {
		Some(u) => u,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !actor.has_permission(&state.db, ADMIN_USERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let target = match User::find_by_id(&state.db, &id).await {
		Ok(Some(u)) => u,
		Ok(None) => return ErrorBuilder::new(ErrorCode::UserNotFound).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch user for password reset: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	if target.auth_method != "local" {
		return ErrorBuilder::new(ErrorCode::ExternalAuthRequired).build();
	}

	if let Err(code) = validate_password(&req.password) {
		return ErrorBuilder::new(code).build();
	}

	let password_hash = match hash_password(&req.password) {
		Ok(h) => h,
		Err(e) => {
			eprintln!("[USERS] Failed to hash password: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	if let Err(e) = target.set_password(&state.db, &password_hash).await {
		eprintln!("[USERS] Failed to update password: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::PasswordChanged, Some(actor.id), Some(EntityType::User), Some(id));

	ResponseBuilder::new(ResponseBody::Json(crate::types::MessageResponse {
		message: "Password updated successfully".to_string(),
	}))
	.build()
}
