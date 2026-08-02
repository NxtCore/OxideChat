use crate::ai;
use crate::types::{GatewayAuthContext, INFERENCE_READ_SCOPE, INFERENCE_WRITE_SCOPE, JobState};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, SkinContext, SkinRequestMetadata, openai_error_response};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub async fn authenticate(pool: &PgPool, headers: &HeaderMap, scope: &str) -> Result<GatewayAuthContext, Response> {
	let Some(value) = headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) else {
		return Err(error_response(
			StatusCode::UNAUTHORIZED,
			"Missing bearer authentication",
			"authentication_error",
			"invalid_api_key",
		));
	};
	let Some((scheme, token)) = value.split_once(' ') else {
		return Err(error_response(
			StatusCode::UNAUTHORIZED,
			"Invalid bearer authentication",
			"authentication_error",
			"invalid_api_key",
		));
	};
	if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
		return Err(error_response(
			StatusCode::UNAUTHORIZED,
			"Invalid bearer authentication",
			"authentication_error",
			"invalid_api_key",
		));
	}
	let context = crate::types::GatewayCredential::authenticate(pool, token).await.map_err(|error| match error {
		crate::types::GatewayAuthError::Invalid => error_response(StatusCode::UNAUTHORIZED, "Incorrect API key provided", "authentication_error", "invalid_api_key"),
		crate::types::GatewayAuthError::Unavailable => error_response(
			StatusCode::INTERNAL_SERVER_ERROR,
			"Authentication service unavailable",
			"server_error",
			"gateway_auth_unavailable",
		),
	})?;
	if !context.allows(scope) {
		return Err(error_response(
			StatusCode::FORBIDDEN,
			"API key is not permitted to use this endpoint",
			"permission_error",
			"insufficient_scope",
		));
	}
	Ok(context)
}

pub async fn run_chat(request: OpenAIChatRequest, metadata: SkinRequestMetadata) -> Response {
	let engine = ai::get();
	let engine = engine.read().await;
	let context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	OpenAIChatSkin::handle_chat(State(context), Some(axum::Extension(metadata)), SkinAwareJson(request)).await
}

pub async fn run_responses(request: OpenAIResponsesRequestPayload, metadata: SkinRequestMetadata) -> Response {
	let engine = ai::get();
	let engine = engine.read().await;
	let context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	OpenAIResponsesSkin::handle_responses(State(context), Some(axum::Extension(metadata)), SkinAwareJson(request)).await
}

pub async fn authenticate_read(State(state): State<Arc<JobState>>, request: Request, next: Next) -> Response {
	authenticate_request(&state, request, next, INFERENCE_READ_SCOPE).await
}

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

pub async fn add_request_id(request: Request, next: Next) -> Response {
	let request_id = Uuid::new_v4();
	let response = next.run(request).await;
	add_request_id_header(response, request_id)
}

#[must_use]
pub fn error_response(status: StatusCode, message: impl Into<String>, kind: impl Into<String>, code: impl Into<String>) -> Response {
	(status, axum::Json(openai_error_response(message, kind, code))).into_response()
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
