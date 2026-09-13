use crate::types::{AnalyticsQuery, Budget, JobState, RequestContext, UsageEvent};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	extract::{Extension, Query, State},
	response::IntoResponse,
};
use std::sync::Arc;

/// GET /api/v1/users/@me
///
/// Get the current authenticated user.
///
/// # Errors
///
/// Returns 401 if not authenticated.
pub async fn get_me(State(state): State<Arc<JobState>>, Extension(RequestContext { user: current_user }): Extension<RequestContext>) -> impl IntoResponse {
	let user = match current_user {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	match user.to_response(&state.db).await {
		Ok(user_response) => ResponseBuilder::new(ResponseBody::Json(user_response)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch roles for user {}: {e}", user.id);
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn get_my_budget(State(state): State<Arc<JobState>>, Extension(RequestContext { user: current_user }): Extension<RequestContext>) -> impl IntoResponse {
	let Some(user) = current_user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	match Budget::status_for_user(&state.db, &user.id).await {
		Ok(status) => ResponseBuilder::new(ResponseBody::Json(status)).build(),
		Err(e) => {
			eprintln!("[USERS] Failed to fetch budget status for user {}: {e}", user.id);
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn get_my_analytics(State(state): State<Arc<JobState>>, Extension(RequestContext { user: current_user }): Extension<RequestContext>, Query(params): Query<AnalyticsQuery>) -> impl IntoResponse {
	let Some(user) = current_user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	let group_by = params.group_by.as_deref().unwrap_or("model");
	if group_by == "day_model" {
		return match UsageEvent::day_model_analytics(&state.db, params.from, params.to, Some(&user.id)).await {
			Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
			Err(e) => {
				eprintln!("[ANALYTICS] Failed to load user day_model analytics: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		};
	}
	match UsageEvent::analytics_for_user(&state.db, &user.id, params.from, params.to, group_by).await {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[ANALYTICS] Failed to load user analytics: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
