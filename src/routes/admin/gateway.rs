use crate::types::consts::{ADMIN_DEVELOPER_ACCESS_EDIT, ADMIN_DEVELOPER_AUDIT_VIEW, ADMIN_DEVELOPER_KEYS_EDIT, ADMIN_DEVELOPER_LIMITS_EDIT, ADMIN_DEVELOPER_VIEW};
use crate::types::{
	AppError, AuthUser, GatewayAdminActionRequest, GatewayAdminError, GatewayAdminKeyDetailResponse, GatewayAdminKeyQuery, GatewayAdminKeyResponse, GatewayAdminRequestQuery,
	GatewayAdminTeamQuery, GatewayAdminTeamResponse, GatewayAuditEntryResponse, GatewayAuditQuery, GatewayEventQuery, GatewayEventResponse, GatewayKeyAdminAction, JobState, PageQuery, RequestLog,
	SetGatewayTeamAccessRequest, SetGatewayTeamLimitsRequest, json_ok,
};
use axum::{
	Json,
	extract::{Path, Query, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/gateway/keys
pub async fn list_keys(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(query): Query<GatewayAdminKeyQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	json_ok(GatewayAdminKeyResponse::list(&state.db, &query, &page).await?)
}

/// GET /api/v1/admin/gateway/keys/{id}
pub async fn get_key(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	json_ok(GatewayAdminKeyDetailResponse::load(&state.db, &id).await?)
}

/// GET /api/v1/admin/gateway/keys/{id}/requests
pub async fn list_key_requests(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Query(page): Query<PageQuery>, Query(query): Query<GatewayAdminRequestQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	let logs = RequestLog::list(&state.db, query.filter(Some(id)), &page)
		.await
		.map_err(GatewayAdminError::from)?;
	json_ok(logs)
}

/// GET /api/v1/admin/gateway/requests
pub async fn list_requests(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(query): Query<GatewayAdminRequestQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	let logs = RequestLog::list(&state.db, query.filter(None), &page)
		.await
		.map_err(GatewayAdminError::from)?;
	json_ok(logs)
}

async fn key_action(state: &JobState, auth: AuthUser, id: Uuid, action: GatewayKeyAdminAction, request: Option<Json<GatewayAdminActionRequest>>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_KEYS_EDIT).await?;
	let reason = request.and_then(|Json(request)| request.reason);
	json_ok(GatewayAdminKeyResponse::apply_action(&state.db, &auth.id, &id, action, reason.as_deref()).await?)
}

/// POST /api/v1/admin/gateway/keys/{id}/revoke
pub async fn revoke_key(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, request: Option<Json<GatewayAdminActionRequest>>) -> Result<Response, AppError> {
	key_action(&state, auth, id, GatewayKeyAdminAction::Revoke, request).await
}

/// POST /api/v1/admin/gateway/keys/{id}/disable
pub async fn disable_key(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, request: Option<Json<GatewayAdminActionRequest>>) -> Result<Response, AppError> {
	key_action(&state, auth, id, GatewayKeyAdminAction::Disable, request).await
}

/// POST /api/v1/admin/gateway/keys/{id}/enable
pub async fn enable_key(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, request: Option<Json<GatewayAdminActionRequest>>) -> Result<Response, AppError> {
	key_action(&state, auth, id, GatewayKeyAdminAction::Enable, request).await
}

/// GET /api/v1/admin/gateway/teams
pub async fn list_teams(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(query): Query<GatewayAdminTeamQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	json_ok(GatewayAdminTeamResponse::list(&state.db, &query, &page).await.map_err(GatewayAdminError::from)?)
}

/// GET /api/v1/admin/gateway/teams/{id}
pub async fn get_team(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	json_ok(GatewayAdminTeamResponse::find(&state.db, &id).await?)
}

/// PUT /api/v1/admin/gateway/teams/{id}/limits
pub async fn set_team_limits(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SetGatewayTeamLimitsRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_LIMITS_EDIT).await?;
	json_ok(GatewayAdminTeamResponse::replace_limits(&state.db, &auth.id, &id, &request).await?)
}

/// PUT /api/v1/admin/gateway/teams/{id}/access
pub async fn set_team_access(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(request): Json<SetGatewayTeamAccessRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_ACCESS_EDIT).await?;
	json_ok(GatewayAdminTeamResponse::set_access(&state.db, &auth.id, &id, &request).await?)
}

/// GET /api/v1/admin/gateway/audit
pub async fn list_audit(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(query): Query<GatewayAuditQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_AUDIT_VIEW).await?;
	json_ok(GatewayAuditEntryResponse::list(&state.db, &query, &page).await?)
}

/// GET /api/v1/admin/gateway/events
pub async fn list_events(State(state): State<Arc<JobState>>, auth: AuthUser, Query(query): Query<GatewayEventQuery>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_DEVELOPER_VIEW).await?;
	json_ok(GatewayEventResponse::list(&state.db, &query).await?)
}
