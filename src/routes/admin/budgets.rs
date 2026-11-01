use crate::types::consts::{ADMIN_BUDGETS_EDIT, ADMIN_BUDGETS_VIEW};
use crate::types::{AppError, AuthUser, Budget, BudgetAssignmentRequest, BudgetResetRequest, CreateBudgetRequest, JobState, ListBudgetsQuery, PageQuery, UpdateBudgetRequest, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_budgets(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ListBudgetsQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_VIEW).await?;

	let budgets = Budget::list(&state.db, &page, params.search.as_deref())
		.await
		.map_err(AppError::database)?;
	json_ok(budgets)
}

pub async fn create_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<CreateBudgetRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	if req.name.trim().is_empty() {
		return Err(ErrorCode::ValidationFailed.into());
	}
	if req.amount <= rust_decimal::Decimal::ZERO {
		return Err(ErrorCode::ValidationFailed.into());
	}
	if !req.has_valid_enums() {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let budget = Budget::create(&state.db, &auth.id, &req)
		.await
		.map_err(AppError::database)?;
	Ok(ResponseBuilder::new(ResponseBody::Json(crate::types::BudgetResponse::from(budget))).status(StatusCode::CREATED).build())
}

pub async fn update_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateBudgetRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	if req.amount.is_some_and(|amount| amount <= rust_decimal::Decimal::ZERO) {
		return Err(ErrorCode::ValidationFailed.into());
	}
	if !req.has_valid_enums() {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let budget = Budget::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let updated = budget
		.update(&state.db, &auth.id, &req)
		.await
		.map_err(AppError::database)?;
	json_ok(crate::types::BudgetResponse::from(updated))
}

pub async fn delete_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	let budget = Budget::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	budget
		.delete(&state.db, &auth.id)
		.await
		.map_err(AppError::database)?;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn assign_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<BudgetAssignmentRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	if req.team_id.is_none() && req.user_id.is_none() {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let budget = Budget::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	budget
		.assign(&state.db, &auth.id, &req)
		.await
		.map_err(AppError::database)?;
	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).build())
}

pub async fn get_budget_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_VIEW).await?;

	let assignments = Budget::list_assignments(&state.db, &id)
		.await
		.map_err(AppError::database)?;
	json_ok(assignments)
}

pub async fn delete_assignment(State(state): State<Arc<JobState>>, auth: AuthUser, Path((budget_id, assignment_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	Budget::delete_assignment(&state.db, &auth.id, &budget_id, &assignment_id)
		.await
		.map_err(AppError::database)?;
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn user_overview(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_VIEW).await?;

	let rows = Budget::user_overview(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(rows)
}

pub async fn team_overview(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_VIEW).await?;

	let rows = Budget::team_overview(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(rows)
}

pub async fn reset_history(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_VIEW).await?;

	let rows = Budget::reset_history(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(rows)
}

pub async fn reset_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<BudgetResetRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_BUDGETS_EDIT).await?;

	let valid_kind = req.kind.as_deref().is_none_or(|kind| matches!(kind, "pooled" | "per_user"));
	if !valid_kind || (req.assignment_id.is_none() && req.budget_id.is_none() && req.team_id.is_none() && req.user_id.is_none()) {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let row = Budget::reset(&state.db, &req, &auth.id)
		.await
		.map_err(AppError::database)?;
	Ok(ResponseBuilder::new(ResponseBody::Json(row)).status(StatusCode::CREATED).build())
}
