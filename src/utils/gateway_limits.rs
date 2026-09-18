use crate::types::gateway::limiter::{GatewayAdmission, GatewayLimitBackend};
use crate::types::{
	GatewayAdmissionRequest, GatewayAuthContext, GatewayInference, GatewayLimitDimension, GatewayLimitError, GatewayLimitSnapshot, GatewayMetricSnapshot,
	GatewayRouteClass, JobState,
};
use crate::utils::openai_gateway::{error_response, translation};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use omniference::skins::{RateLimitHeaders, RateLimitMetric, openai_rate_limit_headers};
use std::time::Duration;

/// Admits an authenticated request and renders local OpenAI errors.
///
/// # Errors
/// Returns a translated rejection response when admission fails.
pub async fn admit(state: &JobState, context: &GatewayAuthContext, class: GatewayRouteClass, tokens: u64, route: &str) -> Result<GatewayAdmission, Response> {
	state
		.gateway_limiter
		.admit(GatewayAdmissionRequest {
			project_id: context.project_id,
			key_id: context.key_id,
			policy: context.policy.clone(),
			route_class: class,
			tokens,
		})
		.await
		.map_err(|error| {
			let dimension = match &error {
				GatewayLimitError::Rejected { dimension, .. } => Some(dimension),
				GatewayLimitError::Unavailable => None,
			};
			tracing::warn!(project_id = %context.project_id, key_id = %context.key_id, route, ?dimension, "gateway admission rejected");
			limit_error_response(error)
		})
}

#[must_use]
/// Maps neutral admission failures to the enabled OpenAI namespace.
pub fn limit_error_response(error: GatewayLimitError) -> Response {
	match error {
		GatewayLimitError::Unavailable => error_response(
			StatusCode::SERVICE_UNAVAILABLE,
			translation("gateway.errors.limiter_unavailable"),
			"server_error",
			"gateway_limiter_unavailable",
		),
		GatewayLimitError::Rejected {
			dimension,
			snapshot,
			retry_after,
		} => {
			let (key, code) = match dimension {
				GatewayLimitDimension::Requests => ("gateway.errors.limit_requests", "requests_limit_exceeded"),
				GatewayLimitDimension::Tokens => ("gateway.errors.limit_tokens", "tokens_limit_exceeded"),
				GatewayLimitDimension::Concurrency => ("gateway.errors.limit_concurrency", "concurrency_limit_exceeded"),
			};
			attach_headers(
				error_response(StatusCode::TOO_MANY_REQUESTS, translation(key), "rate_limit_error", code),
				&snapshot,
				Some(retry_after),
			)
		}
	}
}

#[must_use]
/// Attaches protocol-rendered rate headers and host concurrency headers.
pub fn attach_headers(mut response: Response, snapshot: &GatewayLimitSnapshot, retry_after: Option<Duration>) -> Response {
	let limits = RateLimitHeaders {
		requests: snapshot.requests.as_ref().map(metric),
		tokens: snapshot.tokens.as_ref().map(metric),
		retry_after,
	};
	for (name, value) in openai_rate_limit_headers(limits) {
		if let (Ok(name), Ok(value)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
			response.headers_mut().insert(name, value);
		}
	}
	if let Some((limit, remaining)) = snapshot.concurrency {
		for (name, value) in [("x-oxide-concurrent-limit", limit), ("x-oxide-concurrent-remaining", remaining)] {
			if let Ok(value) = HeaderValue::try_from(value.to_string()) {
				response.headers_mut().insert(name, value);
			}
		}
	}
	response
}

fn metric(snapshot: &GatewayMetricSnapshot) -> RateLimitMetric {
	RateLimitMetric {
		limit: snapshot.capacity,
		remaining: snapshot.remaining,
		reset_after: snapshot.reset_after,
	}
}

#[must_use]
/// Transfers a successful admission into response-body ownership.
pub fn finish(admission: GatewayAdmission, response: Response) -> Response {
	let response = attach_headers(response, &admission.snapshot, None);
	admission.reservation.attach(response)
}

/// Estimates text conservatively and reserves configured bounds for external input.
///
/// # Errors
/// Returns unavailable if required output or external-input bounds are absent.
pub fn estimate_tokens(request: &serde_json::Value, inference: &GatewayInference, output_max: Option<u64>, choices: u64) -> Result<u64, GatewayLimitError> {
	let output = output_max
		.or(inference.max_output_tokens.map(u64::from))
		.filter(|value| *value > 0)
		.ok_or(GatewayLimitError::Unavailable)?;
	let bytes = serde_json::to_vec(request).map_err(|_| GatewayLimitError::Unavailable)?.len() as u64;
	let input = if has_external_input(request) {
		inference.context_length.map(u64::from).ok_or(GatewayLimitError::Unavailable)?
	} else {
		bytes.saturating_mul(2).saturating_add(1024)
	};
	Ok(input.saturating_add(output).saturating_mul(choices.max(1)))
}

fn has_external_input(value: &serde_json::Value) -> bool {
	match value {
		serde_json::Value::Object(fields) => fields.iter().any(|(key, value)| {
			(matches!(
				key.as_str(),
				"image_url" | "input_audio" | "file_id" | "file_url" | "video_url" | "previous_response_id" | "conversation" | "prompt"
			) && !value.is_null())
				|| has_external_input(value)
		}),
		serde_json::Value::Array(values) => values.iter().any(has_external_input),
		_ => false,
	}
}

/// Resolves and applies the Chat Completions token reservation maximum.
///
/// # Errors
/// Returns a translated error if a conservative reservation cannot be computed.
pub fn chat_tokens(
	request: &mut omniference::types::providers::openai::OpenAIChatRequest,
	context: &GatewayAuthContext,
	inference: &GatewayInference,
) -> Result<u64, Response> {
	if !context.policy.has_token_rules() {
		return Ok(0);
	}
	let output_max = request.max_completion_tokens.or(request.max_tokens).or(inference.max_output_tokens);
	if request.max_completion_tokens.is_none() && request.max_tokens.is_none() {
		request.max_completion_tokens = output_max;
	}
	serde_json::to_value(&*request)
		.map_err(|_| GatewayLimitError::Unavailable)
		.and_then(|value| estimate_tokens(&value, inference, output_max.map(u64::from), u64::from(request.n.unwrap_or(1))))
		.map_err(limit_error_response)
}

/// Resolves and applies the Responses token reservation maximum.
///
/// # Errors
/// Returns a translated error if a conservative reservation cannot be computed.
pub fn responses_tokens(
	request: &mut omniference::types::providers::openai::OpenAIResponsesRequestPayload,
	context: &GatewayAuthContext,
	inference: &GatewayInference,
) -> Result<u64, Response> {
	if !context.policy.has_token_rules() {
		return Ok(0);
	}
	if request.max_output_tokens.is_none() {
		request.max_output_tokens = inference.max_output_tokens.map(i64::from);
	}
	let output_max = request.max_output_tokens.and_then(|value| u64::try_from(value).ok()).filter(|value| *value > 0);
	if output_max.is_none() {
		return Err(limit_error_response(GatewayLimitError::Unavailable));
	}
	serde_json::to_value(&*request)
		.map_err(|_| GatewayLimitError::Unavailable)
		.and_then(|value| estimate_tokens(&value, inference, output_max, 1))
		.map_err(limit_error_response)
}
