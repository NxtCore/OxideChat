use crate::ai;
use crate::i18n::I18n;
use crate::types::{
	API_KEY_ID_HEADER, BUDGET_RESERVATION_METADATA_KEY, PUBLIC_MODEL_METADATA_KEY, GatewayAuthContext, GatewayInference, GatewayModel, GatewayModelAccessError, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE, JobState,
	GatewayRun, RequestLog, RequestLogRecord, RequestLogStatus, RESERVATION_METADATA_KEY, TEAM_ID_HEADER,
};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIChatSkin, OpenAIErrorHandler, OpenAIResponsesSkin, SkinContext, SkinErrorHandler, SkinRequestMetadata, openai_error_response};
use omniference::types::providers::OpenAIModelsResponse;
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

/// Authenticates a bearer credential and verifies the required gateway scope.
///
/// # Errors
///
/// Returns an OpenAI-compatible HTTP response for missing or invalid bearer
/// credentials, unavailable authentication storage, or insufficient scope.
pub async fn authenticate(pool: &PgPool, headers: &HeaderMap, scope: &str) -> Result<GatewayAuthContext, Response> {
	let Some(value) = headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) else {
		return Err(error_response(
			StatusCode::UNAUTHORIZED,
			translation("gateway.errors.auth_missing"),
			"authentication_error",
			"invalid_api_key",
		));
	};
	let Some((scheme, token)) = value.split_once(' ') else {
		return Err(invalid_bearer_response());
	};
	if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
		return Err(invalid_bearer_response());
	}
	let context = crate::types::GatewayCredential::authenticate(pool, token).await.map_err(|error| match error {
		crate::types::GatewayAuthError::Invalid => error_response(
			StatusCode::UNAUTHORIZED,
			translation("gateway.errors.api_key_incorrect"),
			"authentication_error",
			"invalid_api_key",
		),
		crate::types::GatewayAuthError::Unavailable => error_response(
			StatusCode::INTERNAL_SERVER_ERROR,
			translation("gateway.errors.auth_unavailable"),
			"server_error",
			"gateway_auth_unavailable",
		),
	})?;
	if !context.allows(scope) {
		return Err(error_response(
			StatusCode::FORBIDDEN,
			translation("gateway.errors.insufficient_scope"),
			"permission_error",
			"insufficient_scope",
		));
	}
	Ok(context)
}

/// Lists the models visible to an authorized gateway API key.
pub async fn run_models(state: &JobState, context: &GatewayAuthContext) -> Response {
	match GatewayModel::list_for_context(&state.db, context).await {
		Ok(models) => axum::Json(OpenAIModelsResponse {
			object: Some("list".to_string()),
			data: models,
		})
		.into_response(),
		Err(error) => {
			tracing::error!(%error, "failed to list gateway models");
			error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				translation("gateway.errors.list_models"),
				"server_error",
				"internal_error",
			)
		}
	}
}

/// Runs an authorized `OpenAI` chat request.
pub async fn run_chat(request: OpenAIChatRequest, inference: GatewayInference, run: GatewayRun<'_>) -> Response {
	let metadata = request_metadata(run.context, &inference, run.reservation_id, run.usage_type);
	let engine = ai::get();
	let engine = engine.read().await;
	let skin_context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	let response = OpenAIChatSkin::handle_chat(State(skin_context), Some(axum::Extension(metadata)), SkinAwareJson(request)).await;
	log_upstream_failure(run.db, response.status(), run.context, &inference, run.usage_type).await;
	inference.finish(response)
}

/// Runs an authorized `OpenAI` responses request.
pub async fn run_responses(request: OpenAIResponsesRequestPayload, inference: GatewayInference, run: GatewayRun<'_>) -> Response {
	let metadata = request_metadata(run.context, &inference, run.reservation_id, run.usage_type);
	let raw_request = match serde_json::to_value(request) {
		Ok(raw_request) => raw_request,
		Err(error) => {
			tracing::error!(%error, "failed to convert OpenAI responses request");
			return error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				translation("gateway.errors.request_conversion"),
				"server_error",
				"internal_error",
			);
		}
	};
	let engine = ai::get();
	let engine = engine.read().await;
	let skin_context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	let response = OpenAIResponsesSkin::handle_responses(State(skin_context), Some(axum::Extension(metadata)), SkinAwareJson(raw_request)).await;
	log_upstream_failure(run.db, response.status(), run.context, &inference, run.usage_type).await;
	inference.finish(response)
}

/// Records every upstream server failure as a request log row.
async fn log_upstream_failure(db: &PgPool, status: StatusCode, context: &GatewayAuthContext, inference: &GatewayInference, usage_type: &str) {
	if !status.is_server_error() {
		return;
	}
	tracing::error!(status = status.as_u16(), key_id = %context.key_id, team_id = %context.team_id, usage_type, "gateway inference request failed upstream");
	let record = RequestLogRecord {
		user_id: context.user_id,
		team_id: Some(context.team_id),
		api_key_id: Some(context.key_id),
		model_id: Some(inference.model_id),
		public_model_id: &inference.public_model_id,
		request_type: usage_type,
		status: RequestLogStatus::Error,
		status_code: Some(i32::from(status.as_u16())),
		input_tokens: 0,
		output_tokens: 0,
		reasoning_tokens: 0,
		cost_total: rust_decimal::Decimal::ZERO,
	};
	if let Err(error) = RequestLog::record(db, record).await {
		tracing::warn!(%error, "failed to record gateway request failure");
	}
}

/// Axum middleware requiring gateway model-read permission.
pub async fn authenticate_read(State(state): State<Arc<JobState>>, request: Request, next: Next) -> Response {
	authenticate_request(&state, request, next, INFERENCE_READ_SCOPE).await
}

/// Axum middleware requiring gateway inference-write permission.
pub async fn authenticate_write(State(state): State<Arc<JobState>>, request: Request, next: Next) -> Response {
	authenticate_request(&state, request, next, INFERENCE_WRITE_SCOPE).await
}

async fn authenticate_request(state: &JobState, mut request: Request, next: Next, scope: &str) -> Response {
	let ip = crate::middleware::client_ip(&request);
	let prefix = bearer_token_prefix(&request);
	if let Some(response) = pre_auth_rate_limited(ip, prefix).await {
		return response;
	}
	let context = match authenticate(&state.db, request.headers(), scope).await {
		Ok(context) => context,
		Err(response) => return response,
	};
	let headers = gateway_context_headers(&context);
	request.extensions_mut().insert(context);
	let mut response = next.run(request).await;
	response.headers_mut().extend(headers);
	response
}

/// Rate limits authentication attempts before any Argon2 verification runs.
///
/// Limits apply per client IP and per bearer-token key prefix, so unauthenticated
/// callers cannot force expensive password verifications at high rates.
async fn pre_auth_rate_limited(ip: String, prefix: Option<String>) -> Option<Response> {
	let limiter = crate::utils::rate_limit::global();
	let window = crate::utils::rate_limit::RATE_LIMIT_WINDOW;
	let ip_allowed = limiter.allow(&format!("gateway:ip:{ip}"), crate::utils::rate_limit::gateway_ip_limit(), window).await;
	let key_allowed = match prefix {
		Some(prefix) => limiter.allow(&format!("gateway:key:{prefix}"), crate::utils::rate_limit::gateway_key_limit(), window).await,
		None => true,
	};
	if ip_allowed && key_allowed {
		return None;
	}
	let mut response = error_response(
		StatusCode::TOO_MANY_REQUESTS,
		translation("gateway.errors.rate_limited"),
		"rate_limit_error",
		"rate_limit_exceeded",
	);
	if let Ok(value) = HeaderValue::from_str(&window.as_secs().to_string()) {
		response.headers_mut().insert(header::RETRY_AFTER, value);
	}
	Some(response)
}

/// Extracts the `oxc_<key-id>` prefix of a bearer token, used as a rate-limit key.
#[must_use]
pub(crate) fn bearer_token_prefix(request: &Request) -> Option<String> {
	let value = request.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
	let (_, token) = value.split_once(' ')?;
	let remainder = token.strip_prefix("oxc_")?;
	let key_id = remainder.split('_').next()?;
	(!key_id.is_empty()).then(|| format!("oxc_{key_id}"))
}

/// Axum middleware assigning a unique request identifier response header.
pub async fn add_request_id(request: Request, next: Next) -> Response {
	let request_id = Uuid::new_v4();
	let response = next.run(request).await;
	add_request_id_header(response, request_id)
}

/// Builds an OpenAI-compatible JSON error response.
#[must_use]
pub fn error_response(status: StatusCode, message: impl Into<String>, kind: impl Into<String>, code: impl Into<String>) -> Response {
	(status, axum::Json(openai_error_response(message, kind, code))).into_response()
}

/// Converts model-policy failures into OpenAI-compatible responses.
#[must_use]
pub fn model_access_error_response(error: GatewayModelAccessError, requested_id: &str) -> Response {
	match error {
		GatewayModelAccessError::NotFound => OpenAIErrorHandler.handle_model_not_found(requested_id),
		GatewayModelAccessError::BudgetExceeded => error_response(
			StatusCode::TOO_MANY_REQUESTS,
			translation("gateway.errors.budget_exceeded"),
			"insufficient_quota",
			"budget_exceeded",
		),
		GatewayModelAccessError::BudgetUnavailable => error_response(
			StatusCode::SERVICE_UNAVAILABLE,
			translation("gateway.errors.budget_unavailable"),
			"server_error",
			"gateway_budget_unavailable",
		),
		GatewayModelAccessError::Database(error) => {
			tracing::error!(%error, model_id = requested_id, "failed to evaluate gateway model and budget policy");
			error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				translation("gateway.errors.model_resolve"),
				"server_error",
				"internal_error",
			)
		}
	}
}

/// Returns a translation using the configured server language.
#[must_use]
pub fn translation(key: &str) -> String {
	I18n::get().translate(key, &None)
}

fn invalid_bearer_response() -> Response {
	error_response(
		StatusCode::UNAUTHORIZED,
		translation("gateway.errors.auth_invalid"),
		"authentication_error",
		"invalid_api_key",
	)
}

fn request_metadata(context: &GatewayAuthContext, inference: &GatewayInference, reservation_id: Uuid, usage_type: &str) -> SkinRequestMetadata {
	let mut metadata = BTreeMap::new();
	metadata.insert(RESERVATION_METADATA_KEY.to_string(), reservation_id.to_string());
	if let Some(budget_id) = inference.budget_reservation_id() {
		metadata.insert(BUDGET_RESERVATION_METADATA_KEY.to_string(), budget_id.to_string());
	}
	metadata.insert("oxide_user_id".to_string(), context.user_id.to_string());
	metadata.insert("oxide_api_key_id".to_string(), context.key_id.to_string());
	metadata.insert("oxide_model_id".to_string(), inference.model_id.to_string());
	metadata.insert(PUBLIC_MODEL_METADATA_KEY.to_string(), inference.public_model_id.clone());
	metadata.insert("oxide_team_id".to_string(), context.team_id.to_string());
	metadata.insert("oxide_request_type".to_string(), usage_type.to_string());
	SkinRequestMetadata(metadata)
}

#[must_use]
fn add_request_id_header(mut response: Response, request_id: Uuid) -> Response {
	let (name, value) = omniference::skins::openai_request_id_header(&request_id.to_string());
	if let Ok(value) = HeaderValue::from_str(&value) {
		response.headers_mut().insert(name, value);
	}
	response
}

#[must_use]
fn gateway_context_headers(context: &GatewayAuthContext) -> HeaderMap {
	let mut headers = HeaderMap::with_capacity(2);
	if let Ok(value) = HeaderValue::from_str(&context.key_id.to_string()) {
		headers.insert(API_KEY_ID_HEADER, value);
	}
	if let Ok(value) = HeaderValue::from_str(&context.team_id.to_string()) {
		headers.insert(TEAM_ID_HEADER, value);
	}
	headers
}
