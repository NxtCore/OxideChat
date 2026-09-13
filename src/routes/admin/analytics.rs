use crate::types::consts::ADMIN_ANALYTICS_VIEW;
use crate::types::{AnalyticsQuery, JobState, RequestContext, UsageEvent};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	extract::{Extension, Query, State},
	response::IntoResponse,
};
use std::sync::Arc;

pub async fn get_analytics(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user: current_user }): Extension<RequestContext>,
	Query(params): Query<AnalyticsQuery>,
) -> impl IntoResponse {
	let Some(user) = current_user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_ANALYTICS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let group_by = params.group_by.as_deref().unwrap_or("model");
	if group_by == "day_model" {
		return match UsageEvent::day_model_analytics(&state.db, params.from, params.to, params.user_id.as_ref()).await {
			Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
			Err(e) => {
				eprintln!("[ANALYTICS] Failed to load day_model analytics: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		};
	}
	let analytics = match params.user_id.as_ref() {
		Some(user_id) => UsageEvent::analytics_for_user(&state.db, user_id, params.from, params.to, group_by).await,
		None => UsageEvent::analytics(&state.db, params.from, params.to, group_by).await,
	};
	match analytics {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[ANALYTICS] Failed to load analytics: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
