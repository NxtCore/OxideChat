use crate::types::consts::{ADMIN_TEAMS_EDIT, ADMIN_TEAMS_VIEW};
use crate::types::{AppError, AuthUser, CreateTeamRequest, JobState, ListTeamsQuery, PageQuery, Team, UpdateTeamBudgetRequest, UpdateTeamMemberRoleRequest, UpdateTeamMembersRequest, UpdateTeamModelsRequest, UpdateTeamRequest, json_ok};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_teams(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ListTeamsQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_VIEW).await?;

	let teams = Team::list_paginated(&state.db, &page, params.search.as_deref())
		.await
		.map_err(AppError::database)?;
	json_ok(teams)
}

pub async fn create_team(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<CreateTeamRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	if req.name.trim().is_empty() {
		return Err(ErrorCode::ValidationFailed.into());
	}

	let team = Team::create(&state.db, &req)
		.await
		.map_err(AppError::database)?;
	let response = team
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

pub async fn get_team(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_VIEW).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let response = team
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}

pub async fn update_team(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let updated = team
		.update(&state.db, &req)
		.await
		.map_err(AppError::database)?;
	let response = updated
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}

pub async fn delete_team(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let deleted = team
		.delete(&state.db)
		.await
		.map_err(AppError::database)?;
	if !deleted {
		return Err(ErrorCode::Forbidden.into());
	}
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn set_team_members(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamMembersRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	team.set_members(&state.db, &req.user_ids)
		.await
		.map_err(AppError::database)?;
	let response = team
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}

pub async fn set_team_models(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamModelsRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	team.update_model_access(&state.db, &auth.id, &req.provider_ids, &req.model_ids)
		.await
		.map_err(AppError::database)?;
	let response = team
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}

pub async fn update_team_budget(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamBudgetRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let updated = team
		.update_budget(&state.db, &auth.id, &req)
		.await
		.map_err(AppError::database)?;
	let response = updated
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}

pub async fn set_team_member_role(State(state): State<Arc<JobState>>, auth: AuthUser, Path((id, user_id)): Path<(Uuid, Uuid)>, Json(req): Json<UpdateTeamMemberRoleRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_TEAMS_EDIT).await?;

	let team = Team::find_by_id(&state.db, &id)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::NotFound)?;
	let changed = team
		.set_member_role(&state.db, &auth.id, &user_id, req.role)
		.await
		.map_err(AppError::database)?;
	if !changed {
		return Err(ErrorCode::NotFound.into());
	}
	let response = team
		.detailed_response(&state.db)
		.await
		.map_err(AppError::database)?;
	json_ok(response)
}
