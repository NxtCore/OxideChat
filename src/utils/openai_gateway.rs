use crate::ai;
use crate::i18n::I18n;
use crate::utils::gateway_limits::limit_error_response;
use crate::types::{
	API_KEY_ID_HEADER, BUDGET_RESERVATION_METADATA_KEY, GatewayAuthContext, GatewayInference, GatewayLimitError, GatewayModel, GatewayModelAccessError, GatewayRequestScope, GatewayRoute,
	GatewayRun, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE, JobState, PUBLIC_MODEL_METADATA_KEY, REQUEST_ID_METADATA_KEY, RESERVATION_METADATA_KEY, RequestLog, RequestLogContext,
	RequestLogErrorCategory, RequestLogFailure, RequestLogOwner, RequestLogTrace, STARTED_AT_METADATA_KEY, STREAMING_METADATA_KEY, TEAM_ID_HEADER,
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

async fn authenticate_owned(pool: &PgPool, headers: &HeaderMap, scope: &str) -> Result<GatewayAuthContext, (Response, Option<RequestLogOwner>)> {
	let Some(value) = headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) else {
		return Err((
			error_response(
				StatusCode::UNAUTHORIZED,
				translation("gateway.errors.auth_missing"),
				"authentication_error",
				"invalid_api_key",
			),
			None,
		));
	};
	let Some((scheme, token)) = value.split_once(' ') else {
		return Err((invalid_bearer_response(), None));
	};
	if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
		return Err((invalid_bearer_response(), None));
	}
	let context = crate::types::GatewayCredential::authenticate_with_owner(pool, token)
		.await
		.map_err(|failure| {
			let response = match failure.error {
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
			};
			(response, failure.owner)
		})?;
	if !context.allows(scope) {
		let response = error_response(
			StatusCode::FORBIDDEN,
			translation("gateway.errors.insufficient_scope"),
			"permission_error",
			"insufficient_scope",
		);
		return Err((response, Some(context.owner())));
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
	let metadata = request_metadata(&run, &inference);
	let engine = ai::get();
	let engine = engine.read().await;
	let skin_context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	let response = OpenAIChatSkin::handle_chat(State(skin_context), Some(axum::Extension(metadata)), SkinAwareJson(request)).await;
	log_upstream_failure(&run.scope, &inference, response.status()).await;
	inference.finish(response)
}

/// Runs an authorized `OpenAI` responses request.
pub async fn run_responses(request: OpenAIResponsesRequestPayload, inference: GatewayInference, run: GatewayRun<'_>) -> Response {
	let metadata = request_metadata(&run, &inference);
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
	log_upstream_failure(&run.scope, &inference, response.status()).await;
	inference.finish(response)
}

/// Records every upstream server failure as a request log row.
async fn log_upstream_failure(scope: &GatewayRequestScope<'_>, inference: &GatewayInference, status: StatusCode) {
	if !status.is_server_error() {
		return;
	}
	let context = scope.context;
	tracing::error!(status = status.as_u16(), key_id = %context.key_id, team_id = %context.team_id, usage_type = scope.usage_type, "gateway inference request failed upstream");
	let failure = RequestLogFailure {
		category: RequestLogErrorCategory::Upstream,
		status_code: Some(i32::from(status.as_u16())),
	};
	log_request_failure(scope.db, &scope.inference_trace(inference), failure).await;
}

/// Stores a failed request's log row; storage problems are logged and never fail the request.
pub async fn log_request_failure(db: &PgPool, trace: &RequestLogTrace<'_>, failure: RequestLogFailure) {
	if let Err(error) = RequestLog::record_failure(db, trace, failure).await {
		tracing::warn!(%error, category = failure.category.as_str(), "failed to record gateway request failure");
	}
}

/// Answers a model or budget refusal and logs it for the key's owner.
pub async fn reject_model_access(scope: &GatewayRequestScope<'_>, inference: Option<&GatewayInference>, error: GatewayModelAccessError, requested_id: &str) -> Response {
	let category = match &error {
		GatewayModelAccessError::NotFound => Some(RequestLogErrorCategory::ModelAccess),
		GatewayModelAccessError::BudgetExceeded => Some(RequestLogErrorCategory::Budget),
		GatewayModelAccessError::BudgetUnavailable | GatewayModelAccessError::Database(_) => None,
	};
	let response = model_access_error_response(error, requested_id);
	if let Some(category) = category {
		let trace = inference.map_or_else(|| scope.trace(requested_id, None, None), |inference| scope.inference_trace(inference));
		let failure = RequestLogFailure {
			category,
			status_code: Some(i32::from(response.status().as_u16())),
		};
		log_request_failure(scope.db, &trace, failure).await;
	}
	response
}

/// Answers a limiter refusal and logs rate limit rejections for the key's owner.
pub async fn reject_limited(scope: &GatewayRequestScope<'_>, inference: &GatewayInference, error: GatewayLimitError) -> Response {
	let rejected = matches!(error, GatewayLimitError::Rejected { .. });
	let response = limit_error_response(error);
	if rejected {
		let failure = RequestLogFailure {
			category: RequestLogErrorCategory::RateLimit,
			status_code: Some(i32::from(response.status().as_u16())),
		};
		log_request_failure(scope.db, &scope.inference_trace(inference), failure).await;
	}
	response
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
	let context = match authenticate_owned(&state.db, request.headers(), scope).await {
		Ok(context) => context,
		Err((response, owner)) => {
			let log = request.extensions().get::<RequestLogContext>().copied();
			if let (Some(owner), Some(log)) = (owner.filter(|_| scope == INFERENCE_WRITE_SCOPE), log) {
				let route = if request.uri().path().ends_with(GatewayRoute::RESPONSES.path) {
					GatewayRoute::RESPONSES
				} else {
					GatewayRoute::CHAT
				};
				log_auth_rejection(&state.db, (log, route), owner, response.status()).await;
			}
			return response;
		}
	};
	let headers = gateway_context_headers(&context);
	request.extensions_mut().insert(context);
	let mut response = next.run(request).await;
	response.headers_mut().extend(headers);
	response
}

async fn log_auth_rejection(db: &PgPool, (log, route): (RequestLogContext, GatewayRoute), owner: RequestLogOwner, status: StatusCode) {
	let trace = RequestLogTrace {
		context: log,
		owner,
		request_type: route.usage_type,
		public_model_id: "",
		model_id: None,
		provider: None,
		streaming: false,
	};
	let failure = RequestLogFailure {
		category: RequestLogErrorCategory::Auth,
		status_code: Some(i32::from(status.as_u16())),
	};
	log_request_failure(db, &trace, failure).await;
}

/// Axum middleware assigning a unique request identifier response header and log context.
pub async fn add_request_id(mut request: Request, next: Next) -> Response {
	let context = RequestLogContext::start();
	request.extensions_mut().insert(context);
	let response = next.run(request).await;
	add_request_id_header(response, context.request_id)
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

fn request_metadata(run: &GatewayRun<'_>, inference: &GatewayInference) -> SkinRequestMetadata {
	let scope = &run.scope;
	let mut metadata = BTreeMap::new();
	metadata.insert(RESERVATION_METADATA_KEY.to_string(), run.reservation_id.to_string());
	if let Some(budget_id) = inference.budget_reservation_id() {
		metadata.insert(BUDGET_RESERVATION_METADATA_KEY.to_string(), budget_id.to_string());
	}
	metadata.insert("oxide_user_id".to_string(), scope.context.user_id.to_string());
	metadata.insert("oxide_api_key_id".to_string(), scope.context.key_id.to_string());
	metadata.insert("oxide_model_id".to_string(), inference.model_id.to_string());
	metadata.insert(PUBLIC_MODEL_METADATA_KEY.to_string(), inference.public_model_id.clone());
	metadata.insert("oxide_team_id".to_string(), scope.context.team_id.to_string());
	metadata.insert("oxide_request_type".to_string(), scope.usage_type.to_string());
	metadata.insert(REQUEST_ID_METADATA_KEY.to_string(), scope.log.request_id.to_string());
	metadata.insert(STREAMING_METADATA_KEY.to_string(), scope.streaming.to_string());
	metadata.insert(STARTED_AT_METADATA_KEY.to_string(), scope.log.started_at.timestamp_millis().to_string());
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
