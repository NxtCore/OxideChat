use crate::routes::public::auth::get_current_user;
use crate::types::consts::ADMIN_ANALYTICS_VIEW;
use crate::types::{AnalyticsQuery, JobState, UsageEvent};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	extract::{Query, State},
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;

pub async fn get_analytics(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<AnalyticsQuery>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_ANALYTICS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let group_by = params.group_by.as_deref().unwrap_or("model");
	if let Some(ref user_id) = params.user_id {
		if group_by == "day_model" {
			return match UsageEvent::day_model_analytics(&state.db, params.from, params.to, Some(user_id)).await {
				Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
				Err(e) => {
					eprintln!("[ANALYTICS] Failed to load day_model analytics: {e}");
					ErrorBuilder::new(ErrorCode::DatabaseError).build()
				}
			};
		}
		return match UsageEvent::analytics_for_user(&state.db, user_id, params.from, params.to, group_by).await {
			Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
			Err(e) => {
				eprintln!("[ANALYTICS] Failed to load user analytics: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		};
	}
	if group_by == "day_model" {
		return match UsageEvent::day_model_analytics(&state.db, params.from, params.to, None).await {
			Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
			Err(e) => {
				eprintln!("[ANALYTICS] Failed to load day_model analytics: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		};
	}
	match UsageEvent::analytics(&state.db, params.from, params.to, group_by).await {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[ANALYTICS] Failed to load analytics: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
