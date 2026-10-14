use crate::types::{
	CreateGatewayKeyRequest, GatewayApiKey, GatewayKeyError, GatewayKeyRotationResponse, GatewayKeyTokenResponse, GatewayTeamResponse, JobState, RequestContext,
	RotateGatewayKeyRequest, SetGatewayKeyLimitsRequest, UpdateGatewayKeyRequest,
};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Extension, Path, State},
	http::{HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

fn error_code(error: &GatewayKeyError) -> ErrorCode {
	match error {
		GatewayKeyError::NotFound => ErrorCode::ApiKeyNotFound,
		GatewayKeyError::Revoked => ErrorCode::ApiKeyRevoked,
		GatewayKeyError::TeamForbidden => ErrorCode::ApiKeyTeamForbidden,
		GatewayKeyError::InvalidName => ErrorCode::ApiKeyNameInvalid,
		GatewayKeyError::InvalidScopes => ErrorCode::ApiKeyScopesInvalid,
		GatewayKeyError::InvalidExpiry => ErrorCode::ApiKeyExpiryInvalid,
		GatewayKeyError::InvalidConcurrency => ErrorCode::ApiKeyConcurrencyInvalid,
		GatewayKeyError::InvalidOverlap => ErrorCode::ApiKeyOverlapInvalid,
		GatewayKeyError::InvalidRule => ErrorCode::ApiKeyRuleInvalid,
		GatewayKeyError::DuplicateRule => ErrorCode::ApiKeyRuleDuplicate,
		GatewayKeyError::Secret | GatewayKeyError::Database(_) => ErrorCode::InternalError,
	}
}

fn failure(error: &GatewayKeyError) -> Response {
	let code = error_code(error);
	if code == ErrorCode::InternalError {
		tracing::error!(%error, "gateway API key management failed");
	}
	ErrorBuilder::new(code).build()
}

fn json_response<T: Serialize>(status: StatusCode, body: T) -> Response {
	ResponseBuilder::new(ResponseBody::Json(body)).status(status).build()
}

fn token_response<T: Serialize>(status: StatusCode, body: T) -> Response {
	ResponseBuilder::new(ResponseBody::Json(body))
		.status(status)
		.header(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))
		.build()
}

/// GET /api/v1/developer/teams
pub async fn list_teams(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayTeamResponse::list_for_user(&state.db, &user.id).await {
		Ok(teams) => json_response(StatusCode::OK, teams),
		Err(error) => failure(&GatewayKeyError::Database(error)),
	}
}

/// GET /api/v1/developer/keys
pub async fn list_keys(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::list_for_user(&state.db, &user.id).await {
		Ok(keys) => json_response(StatusCode::OK, keys.iter().map(GatewayApiKey::to_response).collect::<Vec<_>>()),
		Err(error) => failure(&GatewayKeyError::Database(error)),
	}
}

/// POST /api/v1/developer/keys
pub async fn create_key(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user }): Extension<RequestContext>,
	Json(request): Json<CreateGatewayKeyRequest>,
) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::create(&state.db, &user.id, &request).await {
		Ok((key, token)) => token_response(
			StatusCode::CREATED,
			GatewayKeyTokenResponse {
				key: key.to_response(),
				token,
			},
		),
		Err(error) => failure(&error),
	}
}

/// PATCH /api/v1/developer/keys/{id}
pub async fn update_key(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user }): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	Json(request): Json<UpdateGatewayKeyRequest>,
) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::update(&state.db, &user.id, &id, &request).await {
		Ok(key) => json_response(StatusCode::OK, key.to_response()),
		Err(error) => failure(&error),
	}
}

/// POST /api/v1/developer/keys/{id}/revoke
pub async fn revoke_key(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::revoke(&state.db, &user.id, &id).await {
		Ok(key) => json_response(StatusCode::OK, key.to_response()),
		Err(error) => failure(&error),
	}
}

/// POST /api/v1/developer/keys/{id}/rotate
pub async fn rotate_key(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user }): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	request: Option<Json<RotateGatewayKeyRequest>>,
) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	let overlap_seconds = request.and_then(|Json(request)| request.overlap_seconds);
	match GatewayApiKey::rotate(&state.db, &user.id, &id, overlap_seconds).await {
		Ok((key, previous, token)) => token_response(
			StatusCode::CREATED,
			GatewayKeyRotationResponse {
				key: GatewayKeyTokenResponse {
					key: key.to_response(),
					token,
				},
				previous: previous.to_response(),
			},
		),
		Err(error) => failure(&error),
	}
}

/// GET /api/v1/developer/keys/{id}/limits
pub async fn get_limits(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::list_rules(&state.db, &user.id, &id).await {
		Ok(rules) => json_response(StatusCode::OK, rules),
		Err(error) => failure(&error),
	}
}

/// PUT /api/v1/developer/keys/{id}/limits
pub async fn set_limits(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user }): Extension<RequestContext>,
	Path(id): Path<Uuid>,
	Json(request): Json<SetGatewayKeyLimitsRequest>,
) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::replace_rules(&state.db, &user.id, &id, &request.rules).await {
		Ok(rules) => json_response(StatusCode::OK, rules),
		Err(error) => failure(&error),
	}
}

/// DELETE /api/v1/developer/keys/{id}/limits
pub async fn clear_limits(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};
	match GatewayApiKey::replace_rules(&state.db, &user.id, &id, &[]).await {
		Ok(rules) => json_response(StatusCode::OK, rules),
		Err(error) => failure(&error),
	}
}
