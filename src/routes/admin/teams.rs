use crate::routes::public::auth::get_current_user;
use crate::types::consts::{ADMIN_TEAMS_EDIT, ADMIN_TEAMS_VIEW};
use crate::types::{CreateTeamRequest, JobState, ListTeamsQuery, Team, UpdateTeamBudgetRequest, UpdateTeamMembersRequest, UpdateTeamModelsRequest, UpdateTeamRequest};
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

pub async fn list_teams(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ListTeamsQuery>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match Team::list_paginated(&state.db, params.page.unwrap_or(1), params.size.unwrap_or(50), params.search.as_deref()).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to list teams: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn create_team(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateTeamRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if req.name.trim().is_empty() {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).build();
	}

	match Team::create(&state.db, &req).await {
		Ok(team) => match team.detailed_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build(),
			Err(e) => {
				eprintln!("[TEAMS] Failed to build created team response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Err(e) => {
			eprintln!("[TEAMS] Failed to create team: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn get_team(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.detailed_response(&state.db).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to build team response: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn update_team(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team for update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.update(&state.db, &req).await {
		Ok(updated) => match updated.detailed_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[TEAMS] Failed to build updated team response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Err(e) => {
			eprintln!("[TEAMS] Failed to update team: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn delete_team(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team for delete: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.delete(&state.db).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::Forbidden).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to delete team: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn set_team_members(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(id): Path<Uuid>,
	Json(req): Json<UpdateTeamMembersRequest>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team for members update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.set_members(&state.db, &req.user_ids).await {
		Ok(()) => match team.detailed_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[TEAMS] Failed to build members response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Err(e) => {
			eprintln!("[TEAMS] Failed to update team members: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn set_team_models(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateTeamModelsRequest>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team for model update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.set_model_access(&state.db, &req.provider_ids, &req.model_ids).await {
		Ok(()) => match team.detailed_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[TEAMS] Failed to build model access response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Err(e) => {
			eprintln!("[TEAMS] Failed to update team model access: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}

pub async fn update_team_budget(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(id): Path<Uuid>,
	Json(req): Json<UpdateTeamBudgetRequest>,
) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if !user.has_permission(&state.db, ADMIN_TEAMS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let team = match Team::find_by_id(&state.db, &id).await {
		Ok(Some(team)) => team,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[TEAMS] Failed to fetch team for budget update: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	match team.update_budget(&state.db, &req).await {
		Ok(updated) => match updated.detailed_response(&state.db).await {
			Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
			Err(e) => {
				eprintln!("[TEAMS] Failed to build budget response: {e}");
				ErrorBuilder::new(ErrorCode::DatabaseError).build()
			}
		},
		Err(e) => {
			eprintln!("[TEAMS] Failed to update team budget: {e}");
			ErrorBuilder::new(ErrorCode::DatabaseError).build()
		}
	}
}
