use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::consts::{SETTINGS_SESSIONS_REVOKE, SETTINGS_SESSIONS_VIEW};
use crate::types::{AnalyticsQuery, AppError, AuthUser, Budget, BudgetSubject, JobState, UsageEvent, UsageFilter, User, json_ok};
use crate::utils::auth::SESSION_COOKIE_NAME;
use crate::utils::response::ErrorCode;
use axum::{
	extract::{Path, Query, State},
	response::Response,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

/// GET /api/v1/users/@me
///
/// Get the current authenticated user.
///
/// # Errors
///
/// Returns 401 if not authenticated.
pub async fn get_me(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(user.to_response(&state.db).await?)
}

/// GET /api/v1/users/@me/sessions
pub async fn list_my_sessions(State(state): State<Arc<JobState>>, auth: AuthUser, cookies: Cookies) -> Result<Response, AppError> {
	auth.require(&state.db, SETTINGS_SESSIONS_VIEW).await?;
	let current = cookies.get(SESSION_COOKIE_NAME).and_then(|cookie| Uuid::parse_str(cookie.value()).ok());
	json_ok(User::list_sessions(&state.db, &auth.id, current.as_ref()).await.map_err(AppError::database)?)
}

/// DELETE /api/v1/users/@me/sessions/{id}
pub async fn revoke_my_session(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<String>) -> Result<Response, AppError> {
	auth.require(&state.db, SETTINGS_SESSIONS_REVOKE).await?;
	if !User::revoke_session(&state.db, &auth.id, &id).await.map_err(AppError::database)? {
		return Err(ErrorCode::NotFound.into());
	}
	AuditLog::log(&state.db, LogEvent::SessionRevoked, Some(auth.id), Some(EntityType::Session), None);
	json_ok(serde_json::json!({ "success": true }))
}

pub async fn get_my_budget(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(Budget::status_for(&state.db, BudgetSubject::user(user.id)).await?)
}

pub async fn get_my_analytics(State(state): State<Arc<JobState>>, user: AuthUser, Query(params): Query<AnalyticsQuery>) -> Result<Response, AppError> {
	let group_by = params.group_by.as_deref().unwrap_or("model");
	let filter = UsageFilter {
		user_id: Some(user.id),
		..UsageFilter::default()
	};
	if group_by == "day_model" {
		let rows = UsageEvent::day_model_analytics(&state.db, params.from, params.to, filter).await.map_err(AppError::database)?;
		return json_ok(rows);
	}
	let rows = UsageEvent::analytics(&state.db, params.from, params.to, group_by, filter).await.map_err(AppError::database)?;
	json_ok(rows)
}
