use crate::types::consts::ADMIN_ANALYTICS_VIEW;
use crate::types::{AnalyticsQuery, AppError, AuthUser, JobState, UsageEvent, json_ok};
use axum::{
	extract::{Query, State},
	response::Response,
};
use std::sync::Arc;

pub async fn get_analytics(State(state): State<Arc<JobState>>, auth: AuthUser, Query(params): Query<AnalyticsQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_ANALYTICS_VIEW).await?;

	let group_by = params.group_by.as_deref().unwrap_or("model");
	let filter = params.filter();
	match group_by {
		"day_model" => json_ok(UsageEvent::day_model_analytics(&state.db, params.from, params.to, filter).await.map_err(AppError::database)?),
		"api_key" => json_ok(UsageEvent::api_key_analytics(&state.db, params.from, params.to, filter).await.map_err(AppError::database)?),
		_ => json_ok(UsageEvent::analytics(&state.db, params.from, params.to, group_by, filter).await.map_err(AppError::database)?),
	}
}
