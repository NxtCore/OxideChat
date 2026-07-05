use crate::routes::public::auth::get_current_user;
use crate::types::consts::{ADMIN_BUDGETS_EDIT, ADMIN_BUDGETS_VIEW};
use crate::types::{Budget, BudgetAssignmentRequest, CreateBudgetRequest, JobState, ListBudgetsQuery, UpdateBudgetRequest};
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

pub async fn delete_assignment(State(state): State<Arc<JobState>>, cookies: Cookies, Path((_budget_id, assignment_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Budget::delete_assignment(&state.db, &assignment_id).await {
		Ok(()) => (StatusCode::NO_CONTENT, "").into_response(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to delete assignment: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn unassign_budget(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<BudgetAssignmentRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	if !user.has_permission(&state.db, ADMIN_BUDGETS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	if req.team_id.is_none() && req.user_id.is_none() {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}
	match Budget::unassign(&state.db, &req).await {
		Ok(()) => ResponseBuilder::<()>::new(ResponseBody::Empty).build(),
		Err(e) => {
			eprintln!("[BUDGETS] Failed to unassign budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
