use crate::ai;
use crate::i18n::I18n;
use crate::types::{GatewayAuthContext, GatewayInference, GatewayModelAccessError, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE, JobState};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIChatSkin, OpenAIErrorHandler, OpenAIResponsesSkin, SkinContext, SkinErrorHandler, SkinRequestMetadata, openai_error_response};
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

/// Runs an authorized OpenAI chat request.
pub async fn run_chat(request: OpenAIChatRequest, context: &GatewayAuthContext, inference: GatewayInference) -> Response {
	let metadata = request_metadata(context, &inference);
	let engine = ai::get();
	let engine = engine.read().await;
	let skin_context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	OpenAIChatSkin::handle_chat(State(skin_context), Some(axum::Extension(metadata)), SkinAwareJson(request)).await
}

/// Runs an authorized OpenAI responses request.
pub async fn run_responses(request: OpenAIResponsesRequestPayload, context: &GatewayAuthContext, inference: GatewayInference) -> Response {
	let metadata = request_metadata(context, &inference);
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
	OpenAIResponsesSkin::handle_responses(State(skin_context), Some(axum::Extension(metadata)), SkinAwareJson(raw_request)).await
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
	let context = match authenticate(&state.db, request.headers(), scope).await {
		Ok(context) => context,
		Err(response) => return response,
	};
	request.extensions_mut().insert(context.clone());
	let response = next.run(request).await;
	add_gateway_context_headers(response, &context)
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

fn request_metadata(context: &GatewayAuthContext, inference: &GatewayInference) -> SkinRequestMetadata {
	let mut metadata = BTreeMap::new();
	metadata.insert("oxide_user_id".to_string(), context.user_id.to_string());
	metadata.insert("oxide_project_id".to_string(), context.project_id.to_string());
	metadata.insert("oxide_api_key_id".to_string(), context.key_id.to_string());
	metadata.insert("oxide_model_id".to_string(), inference.model_id.to_string());
	if let Some(team_id) = context.team_id {
		metadata.insert("oxide_team_id".to_string(), team_id.to_string());
	}
	SkinRequestMetadata(metadata)
}

#[must_use]
fn add_request_id_header(mut response: Response, request_id: Uuid) -> Response {
	if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
		response.headers_mut().insert("x-request-id", value);
	}
	response
}

#[must_use]
fn add_gateway_context_headers(mut response: Response, context: &GatewayAuthContext) -> Response {
	if let Ok(value) = HeaderValue::from_str(&context.project_id.to_string()) {
		response.headers_mut().insert("x-oxide-project-id", value);
	}
	if let Ok(value) = HeaderValue::from_str(&context.key_id.to_string()) {
		response.headers_mut().insert("x-oxide-api-key-id", value);
	}
	if let Some(team_id) = context.team_id
		&& let Ok(value) = HeaderValue::from_str(&team_id.to_string())
	{
		response.headers_mut().insert("x-oxide-team-id", value);
	}
	if let Ok(value) = HeaderValue::from_str(&context.project_name) {
		response.headers_mut().insert("x-oxide-project-name", value);
	}
	response
}
