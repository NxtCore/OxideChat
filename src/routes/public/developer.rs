use crate::types::consts::ADMIN_DEVELOPER_KEYS_EDIT;
use crate::types::{
	AppError, AuthUser, CreateGatewayKeyRequest, GatewayApiKey, GatewayAdminError, GatewayKeyRotationResponse, GatewayKeyTokenResponse, GatewayModelResponse, GatewayTeamKeysResponse, GatewayTeamResponse, JobState, KeyActor, PageQuery,
	MAX_REQUEST_LOG_RETENTION_DAYS, RequestLog, RequestLogFilter, RequestLogQuery, RequestLogRetentionResponse, RotateGatewayKeyRequest, SetGatewayKeyBudgetRequest, SetGatewayKeyLimitsRequest,
	SetRequestLogRetentionRequest, UpdateGatewayKeyRequest, json_ok,
};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, Query, State},
	http::{HeaderValue, StatusCode, header},
	response::Response,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

async fn key_actor(state: &JobState, user: &AuthUser) -> Result<KeyActor, AppError> {
	Ok(KeyActor::new(&user.id, user.check_permission(&state.db, ADMIN_DEVELOPER_KEYS_EDIT).await?))
}

async fn request_log_filter(state: &JobState, user: &AuthUser, params: &RequestLogQuery) -> Result<RequestLogFilter, AppError> {
	let installation_admin = user.check_permission(&state.db, ADMIN_DEVELOPER_KEYS_EDIT).await?;
	Ok(params.scoped_filter(&state.db, &user.id, installation_admin).await?)
}

fn token_response<T: Serialize>(status: StatusCode, body: T) -> Result<Response, AppError> {
	Ok(ResponseBuilder::new(ResponseBody::Json(body))
		.status(status)
		.header(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))
		.build())
}

/// GET /api/v1/developer/teams
pub async fn list_teams(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(GatewayTeamResponse::list_for_user(&state.db, &user.id).await?)
}

/// GET /api/v1/developer/models
pub async fn list_models(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	json_ok(GatewayModelResponse::list_for_user(&state.db, &user.id).await?)
}

/// GET /api/v1/developer/team-keys
pub async fn list_team_keys(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayTeamKeysResponse::load(&state.db, actor).await?)
}

/// GET /api/v1/developer/keys
pub async fn list_keys(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let keys = GatewayApiKey::list_for_user(&state.db, &user.id).await?;
	json_ok(keys.iter().map(GatewayApiKey::to_response).collect::<Vec<_>>())
}

/// POST /api/v1/developer/keys
pub async fn create_key(State(state): State<Arc<JobState>>, user: AuthUser, Json(request): Json<CreateGatewayKeyRequest>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	let (key, token) = GatewayApiKey::create(&state.db, actor, &request).await?;
	token_response(StatusCode::CREATED, GatewayKeyTokenResponse { key: key.to_response(), token })
}

/// PATCH /api/v1/developer/keys/{id}
pub async fn update_key(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<UpdateGatewayKeyRequest>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::update(&state.db, actor, &id, &request).await?.to_response())
}

/// POST /api/v1/developer/keys/{id}/revoke
pub async fn revoke_key(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::revoke(&state.db, actor, &id).await?.to_response())
}

/// POST /api/v1/developer/keys/{id}/rotate
pub async fn rotate_key(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, request: Option<Json<RotateGatewayKeyRequest>>) -> Result<Response, AppError> {
	let overlap_seconds = request.and_then(|Json(request)| request.overlap_seconds);
	let actor = key_actor(&state, &user).await?;
	let (key, previous, token) = GatewayApiKey::rotate(&state.db, actor, &id, overlap_seconds).await?;
	token_response(
		StatusCode::CREATED,
		GatewayKeyRotationResponse {
			key: GatewayKeyTokenResponse { key: key.to_response(), token },
			previous: previous.to_response(),
		},
	)
}

/// GET /api/v1/developer/keys/{id}/limits
pub async fn get_limits(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::list_rules(&state.db, actor, &id).await?)
}

/// PUT /api/v1/developer/keys/{id}/limits
pub async fn set_limits(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SetGatewayKeyLimitsRequest>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::replace_rules(&state.db, actor, &id, &request.rules).await?)
}

/// DELETE /api/v1/developer/keys/{id}/limits
pub async fn clear_limits(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::replace_rules(&state.db, actor, &id, &[]).await?)
}

/// GET /api/v1/developer/keys/{id}/budget
pub async fn get_budget(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::budget(&state.db, actor, &id).await?)
}

/// PUT /api/v1/developer/keys/{id}/budget
pub async fn set_budget(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SetGatewayKeyBudgetRequest>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	json_ok(GatewayApiKey::replace_budget(&state.db, actor, &id, &request).await?)
}

/// DELETE /api/v1/developer/keys/{id}/budget
pub async fn clear_budget(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = key_actor(&state, &user).await?;
	GatewayApiKey::clear_budget(&state.db, actor, &id).await?;
	json_ok(Option::<()>::None)
}

/// GET /api/v1/developer/request-logs
pub async fn list_request_logs(State(state): State<Arc<JobState>>, user: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<RequestLogQuery>) -> Result<Response, AppError> {
	let filter = request_log_filter(&state, &user, &params).await?;
	let logs = RequestLog::list(&state.db, filter, &page)
		.await
		.map_err(GatewayAdminError::from)?;
	json_ok(logs)
}

/// GET /api/v1/developer/request-logs/export
pub async fn export_request_logs(State(state): State<Arc<JobState>>, user: AuthUser, Query(params): Query<RequestLogQuery>) -> Result<Response, AppError> {
	let filter = request_log_filter(&state, &user, &params).await?;
	json_ok(RequestLog::list_for_export(&state.db, filter).await.map_err(GatewayAdminError::from)?)
}

/// GET /api/v1/developer/request-logs/retention
pub async fn get_request_log_retention(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let days = RequestLog::retention_days(&state.db, &user.id).await.map_err(GatewayAdminError::from)?;
	json_ok(RequestLogRetentionResponse { retention_days: days })
}

/// PUT /api/v1/developer/request-logs/retention
pub async fn set_request_log_retention(State(state): State<Arc<JobState>>, user: AuthUser, Json(request): Json<SetRequestLogRetentionRequest>) -> Result<Response, AppError> {
	if request.retention_days < 1 || request.retention_days > MAX_REQUEST_LOG_RETENTION_DAYS {
		return Err(ErrorCode::RequestLogRetentionInvalid.into());
	}
	RequestLog::set_retention_days(&state.db, &user.id, request.retention_days).await.map_err(GatewayAdminError::from)?;
	json_ok(RequestLogRetentionResponse { retention_days: request.retention_days })
}
