use crate::ai;
use crate::types::{GatewayAuthContext, OpenAiError, OpenAiErrorResponse};
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, SkinContext};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use sqlx::PgPool;
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

pub async fn run_chat(request: OpenAIChatRequest) -> Response {
	let engine = ai::get();
	let engine = engine.read().await;
	let context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	let response = OpenAIChatSkin::handle_chat(axum07::extract::State(context), SkinAwareJson(request)).await;
	bridge_response(response)
}

pub async fn run_responses(request: OpenAIResponsesRequestPayload) -> Response {
	let engine = ai::get();
	let engine = engine.read().await;
	let context = SkinContext::with_service(engine.service().clone());
	drop(engine);
	let response = OpenAIResponsesSkin::handle_responses(axum07::extract::State(context), SkinAwareJson(request)).await;
	bridge_response(response)
}

pub fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
	serde_json::from_slice(body).map_err(|error| format!("Failed to parse request body: {error}"))
}

#[must_use]
pub fn error_response(status: StatusCode, message: impl Into<String>, kind: impl Into<String>, code: impl Into<String>) -> Response {
	(
		status,
		axum::Json(OpenAiErrorResponse {
			error: OpenAiError {
				message: message.into(),
				kind: kind.into(),
				param: None,
				code: code.into(),
			},
		}),
	)
		.into_response()
}

#[must_use]
pub fn add_gateway_headers(mut response: Response, context: Option<&GatewayAuthContext>, request_id: Uuid) -> Response {
	if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
		response.headers_mut().insert("x-request-id", value);
	}
	if let Some(context) = context {
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
	}
	response
}

pub(crate) fn bridge_response(response: axum07::response::Response) -> Response {
	let (parts, body) = response.into_parts();
	Response::from_parts(parts, Body::new(body))
}
