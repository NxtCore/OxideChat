use crate::types::{AnalyticsQuery, AppError, AuthUser, Budget, BudgetSubject, JobState, UsageEvent, UsageFilter, json_ok};
use axum::{
	extract::{Query, State},
	response::Response,
};
use std::sync::Arc;

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
