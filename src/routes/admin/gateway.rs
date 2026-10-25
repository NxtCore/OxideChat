use crate::types::consts::{ADMIN_DEVELOPER_ACCESS_EDIT, ADMIN_DEVELOPER_AUDIT_VIEW, ADMIN_DEVELOPER_KEYS_EDIT, ADMIN_DEVELOPER_LIMITS_EDIT, ADMIN_DEVELOPER_VIEW};
use crate::types::{
	GatewayAdminActionRequest, GatewayAdminError, GatewayAdminKeyDetailResponse, GatewayAdminKeyQuery, GatewayAdminKeyResponse, GatewayAdminRequestQuery, GatewayAdminTeamQuery,
	GatewayAdminTeamResponse, GatewayAuditEntryResponse, GatewayAuditQuery, GatewayEventQuery, GatewayEventResponse, GatewayKeyAdminAction, JobState, RequestContext, RequestLog,
	SetGatewayTeamAccessRequest, SetGatewayTeamLimitsRequest, User,
};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Extension, Path, Query, State},
	response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

async fn authorize(state: &JobState, user: Option<User>, permission: &str) -> Result<User, Response> {
	let Some(user) = user else {
		return Err(ErrorBuilder::new(ErrorCode::NotAuthenticated).build());
	};
	if !user.has_permission(&state.db, permission).await {
		return Err(ErrorBuilder::new(ErrorCode::InsufficientPermissions).build());
	}
	Ok(user)
}

fn error_code(error: &GatewayAdminError) -> ErrorCode {
	match error {
		GatewayAdminError::KeyNotFound => ErrorCode::ApiKeyNotFound,
		GatewayAdminError::KeyRevoked => ErrorCode::ApiKeyRevoked,
		GatewayAdminError::TeamNotFound => ErrorCode::NotFound,
		GatewayAdminError::ReasonInvalid => ErrorCode::GatewayAdminReasonInvalid,
		GatewayAdminError::InvalidRule => ErrorCode::ApiKeyRuleInvalid,
		GatewayAdminError::DuplicateRule => ErrorCode::ApiKeyRuleDuplicate,
		GatewayAdminError::InvalidConcurrency => ErrorCode::ApiKeyConcurrencyInvalid,
		GatewayAdminError::InvalidQuery => ErrorCode::ValidationFailed,
		GatewayAdminError::Database(_) => ErrorCode::DatabaseError,
	}
}

fn respond<T: Serialize>(result: Result<T, GatewayAdminError>) -> Response {
	match result {
		Ok(body) => ResponseBuilder::new(ResponseBody::Json(body)).build(),
		Err(error) => {
			let code = error_code(&error);
			if code == ErrorCode::DatabaseError {
				tracing::error!(%error, "gateway administration failed");
			}
			ErrorBuilder::new(code).build()
		}
	}
}

macro_rules! guard {
	($state:expr, $context:expr, $permission:expr) => {
		match authorize(&$state, $context.user, $permission).await {
			Ok(user) => user,
			Err(response) => return response,
		}
	};
}

/// GET /api/v1/admin/gateway/keys
pub async fn list_keys(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Query(query): Query<GatewayAdminKeyQuery>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(GatewayAdminKeyResponse::list(&state.db, &query).await)
}

/// GET /api/v1/admin/gateway/keys/{id}
pub async fn get_key(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(GatewayAdminKeyDetailResponse::load(&state.db, &id).await)
}

/// GET /api/v1/admin/gateway/keys/{id}/requests
pub async fn list_key_requests(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	Query(query): Query<GatewayAdminRequestQuery>,
) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(
		RequestLog::list(&state.db, query.filter(Some(id)), query.page.unwrap_or(1), query.limit.unwrap_or(50))
			.await
			.map_err(GatewayAdminError::from),
	)
}

/// GET /api/v1/admin/gateway/requests
pub async fn list_requests(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Query(query): Query<GatewayAdminRequestQuery>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(
		RequestLog::list(&state.db, query.filter(None), query.page.unwrap_or(1), query.limit.unwrap_or(50))
			.await
			.map_err(GatewayAdminError::from),
	)
}

async fn key_action(state: &JobState, context: RequestContext, id: Uuid, action: GatewayKeyAdminAction, request: Option<GatewayAdminActionRequest>) -> Response {
	let user = guard!(state, context, ADMIN_DEVELOPER_KEYS_EDIT);
	let reason = request.and_then(|request| request.reason);
	respond(GatewayAdminKeyResponse::apply_action(&state.db, &user.id, &id, action, reason.as_deref()).await)
}

/// POST /api/v1/admin/gateway/keys/{id}/revoke
pub async fn revoke_key(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	request: Option<Json<GatewayAdminActionRequest>>,
) -> impl IntoResponse {
	key_action(&state, context, id, GatewayKeyAdminAction::Revoke, request.map(|Json(request)| request)).await
}

/// POST /api/v1/admin/gateway/keys/{id}/disable
pub async fn disable_key(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	request: Option<Json<GatewayAdminActionRequest>>,
) -> impl IntoResponse {
	key_action(&state, context, id, GatewayKeyAdminAction::Disable, request.map(|Json(request)| request)).await
}

/// POST /api/v1/admin/gateway/keys/{id}/enable
pub async fn enable_key(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	request: Option<Json<GatewayAdminActionRequest>>,
) -> impl IntoResponse {
	key_action(&state, context, id, GatewayKeyAdminAction::Enable, request.map(|Json(request)| request)).await
}

/// GET /api/v1/admin/gateway/teams
pub async fn list_teams(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Query(query): Query<GatewayAdminTeamQuery>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(GatewayAdminTeamResponse::list(&state.db, &query).await.map_err(GatewayAdminError::from))
}

/// GET /api/v1/admin/gateway/teams/{id}
pub async fn get_team(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(GatewayAdminTeamResponse::find(&state.db, &id).await)
}

/// PUT /api/v1/admin/gateway/teams/{id}/limits
pub async fn set_team_limits(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	Json(request): Json<SetGatewayTeamLimitsRequest>,
) -> impl IntoResponse {
	let user = guard!(state, context, ADMIN_DEVELOPER_LIMITS_EDIT);
	respond(GatewayAdminTeamResponse::replace_limits(&state.db, &user.id, &id, &request).await)
}

/// PUT /api/v1/admin/gateway/teams/{id}/access
pub async fn set_team_access(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	Json(request): Json<SetGatewayTeamAccessRequest>,
) -> impl IntoResponse {
	let user = guard!(state, context, ADMIN_DEVELOPER_ACCESS_EDIT);
	respond(GatewayAdminTeamResponse::set_access(&state.db, &user.id, &id, &request).await)
}

/// GET /api/v1/admin/gateway/audit
pub async fn list_audit(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Query(query): Query<GatewayAuditQuery>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_AUDIT_VIEW);
	respond(GatewayAuditEntryResponse::list(&state.db, &query).await)
}

/// GET /api/v1/admin/gateway/events
pub async fn list_events(State(state): State<Arc<JobState>>, Extension(context): Extension<RequestContext>, Query(query): Query<GatewayEventQuery>) -> impl IntoResponse {
	guard!(state, context, ADMIN_DEVELOPER_VIEW);
	respond(GatewayEventResponse::list(&state.db, &query).await)
}
