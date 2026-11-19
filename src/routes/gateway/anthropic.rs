//! Anthropic Messages gateway glue: `x-api-key` authentication and Anthropic error bodies.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use omniference::skins::anthropic::anthropic_error_body;
use serde_json::Value;

const ERROR_BODY_LIMIT: usize = 64 * 1024;

/// Accepts the key as `x-api-key`, as Anthropic clients such as Claude Code send it, and renders
/// the gateway's own `OpenAI`-shaped refusals as Anthropic error bodies.
pub async fn anthropic_compat(mut request: Request, next: Next) -> Response {
	if !request.headers().contains_key(header::AUTHORIZATION)
		&& let Some(value) = request.headers().get("x-api-key").and_then(|key| HeaderValue::from_str(&format!("Bearer {}", key.to_str().ok()?)).ok())
	{
		request.headers_mut().insert(header::AUTHORIZATION, value);
	}
	let response = next.run(request).await;
	let json = response.headers().get(header::CONTENT_TYPE).is_some_and(|value| value.as_bytes().starts_with(b"application/json"));
	if response.status().is_success() || !json {
		return response;
	}
	let (mut parts, body) = response.into_parts();
	parts.headers.remove(header::CONTENT_LENGTH);
	let error_type = error_type(parts.status);
	let Ok(bytes) = to_bytes(body, ERROR_BODY_LIMIT).await else {
		return Response::from_parts(parts, Body::from(anthropic_error_body(error_type, "").to_string()));
	};
	let Some(message) = openai_error_message(&bytes) else {
		return Response::from_parts(parts, Body::from(bytes));
	};
	Response::from_parts(parts, Body::from(anthropic_error_body(error_type, message).to_string()))
}

/// The message of an `OpenAI` error body; `None` for anything else, such as an Anthropic error the skin already rendered.
fn openai_error_message(bytes: &[u8]) -> Option<String> {
	let body: Value = serde_json::from_slice(bytes).ok()?;
	if body.get("type").is_some() {
		return None;
	}
	Some(body.get("error")?.get("message")?.as_str()?.to_owned())
}

fn error_type(status: StatusCode) -> &'static str {
	match status.as_u16() {
		400 | 422 => "invalid_request_error",
		401 => "authentication_error",
		402 => "billing_error",
		403 => "permission_error",
		404 => "not_found_error",
		413 => "request_too_large",
		429 => "rate_limit_error",
		503 | 529 => "overloaded_error",
		504 => "timeout_error",
		_ => "api_error",
	}
}
