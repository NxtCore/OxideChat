use crate::routes::public::auth::get_current_user;
use crate::types::consts::{ADMIN_BUDGETS_EDIT, ADMIN_BUDGETS_VIEW};
use crate::types::{Budget, BudgetAssignmentRequest, BudgetResetRequest, CreateBudgetRequest, JobState, ListBudgetsQuery, UpdateBudgetRequest};
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

pub async fn list_budgets(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ListBudgetsQuery>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::list(&state.db, params.page.unwrap_or(1), params.size.unwrap_or(50), params.search.as_deref()).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to list budgets: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn create_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateBudgetRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	if req.name.trim().is_empty() {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}
	match Budget::create(&state.db, &req).await {
		Ok(budget) => ResponseBuilder::new(ResponseBody::Json(crate::types::BudgetResponse::from(budget)))
			.status(StatusCode::CREATED)
			.build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to create budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn update_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateBudgetRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let budget = match Budget::find_by_id(&state.db, &id).await {
		Ok(Some(budget)) => budget,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to fetch budget: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};
	match budget.update(&state.db, &req).await {
		Ok(updated) => ResponseBuilder::new(ResponseBody::Json(crate::types::BudgetResponse::from(updated))).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to update budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn delete_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let budget = match Budget::find_by_id(&state.db, &id).await {
		Ok(Some(budget)) => budget,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to fetch budget for delete: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};
	match budget.delete(&state.db).await {
		Ok(()) => (StatusCode::NO_CONTENT, "").into_response(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to delete budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn assign_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<BudgetAssignmentRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let budget = match Budget::find_by_id(&state.db, &id).await {
		Ok(Some(budget)) => budget,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to fetch budget for assignment: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};
	match budget.assign(&state.db, &req).await {
		Ok(()) => ResponseBuilder::<()>::new(ResponseBody::Empty).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to assign budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn get_budget_assignments(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::list_assignments(&state.db, &id).await {
		Ok(assignments) => ResponseBuilder::new(ResponseBody::Json(assignments)).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to list assignments: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn delete_assignment(State(state): State<Arc<JobState>>, cookies: Cookies, Path((budget_id, assignment_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::delete_assignment(&state.db, &budget_id, &assignment_id).await {
		Ok(()) => (StatusCode::NO_CONTENT, "").into_response(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to delete assignment: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}


pub async fn user_overview(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::user_overview(&state.db).await {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to load user overview: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn team_overview(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::team_overview(&state.db).await {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to load team overview: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn reset_history(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::reset_history(&state.db).await {
		Ok(rows) => ResponseBuilder::new(ResponseBody::Json(rows)).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to load reset history: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn reset_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<BudgetResetRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let valid_kind = req.kind.as_deref().map_or(true, |kind| matches!(kind, "pooled" | "per_user"));
	if !valid_kind || (req.assignment_id.is_none() && req.budget_id.is_none() && req.team_id.is_none() && req.user_id.is_none()) {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}
	match Budget::reset(&state.db, &req, &user.id).await {
		Ok(row) => ResponseBuilder::new(ResponseBody::Json(row)).status(StatusCode::CREATED).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to reset budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
