use crate::types::{
	CONCURRENT_LIMIT_HEADER, CONCURRENT_REMAINING_HEADER, GatewayAdmission, GatewayAuthContext, GatewayLimitBackend, GatewayLimitDimension, GatewayLimitError,
	GatewayLimitSnapshot, GatewayMetricSnapshot, GatewayRoute, JobState,
};
use crate::routes::gateway::{error_response, translation};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use omniference::skins::{RateLimitHeaders, RateLimitMetric, openai_rate_limit_headers};
use std::future::Future;
use std::time::Duration;
use uuid::Uuid;

/// Admits a request, runs it with its reservation id, and ties the reservation to the response body.
///
/// A failed token estimate or admission is rendered here as a translated `OpenAI` error.
/// A non-success response refunds its token reservation because providers report no usage for it.
pub async fn run_limited<F, Fut>(state: &JobState, context: &GatewayAuthContext, route: GatewayRoute, tokens: Result<u64, GatewayLimitError>, run: F) -> Response
where
	F: FnOnce(Uuid) -> Fut,
	Fut: Future<Output = Response>,
{
	match admit(state, context, route, tokens).await {
		Ok(admission) => {
			let response = run(admission.reservation.id).await;
			complete(state, admission, response)
		}
		Err(error) => limit_error_response(error),
	}
}

/// Admits a request against the limiter, logging any rejection.
///
/// # Errors
///
/// Returns the failed token estimate or the limiter's rejection.
pub async fn admit(state: &JobState, context: &GatewayAuthContext, route: GatewayRoute, tokens: Result<u64, GatewayLimitError>) -> Result<GatewayAdmission, GatewayLimitError> {
	let admitted = match tokens {
		Ok(tokens) => state.gateway_limiter.admit(context.admission_request(route, tokens)).await,
		Err(error) => Err(error),
	};
	if let Err(error) = &admitted {
		log_failure(error, context, route);
	}
	admitted
}

/// Ties an admitted request's reservation to its response, refunding it when the response failed.
pub fn complete(state: &JobState, admission: GatewayAdmission, response: Response) -> Response {
	if !response.status().is_success() {
		state.gateway_limiter.reconcile(admission.reservation.id, 0);
	}
	finish(admission, response)
}

#[allow(clippy::cognitive_complexity)]
fn log_failure(error: &GatewayLimitError, context: &GatewayAuthContext, route: GatewayRoute) {
	let (team_id, key_id, path) = (context.team_id, context.key_id, route.path);
	match error {
		GatewayLimitError::Rejected { dimension, .. } => tracing::warn!(%team_id, %key_id, path, ?dimension, "gateway admission rejected"),
		GatewayLimitError::Overloaded => tracing::warn!(%team_id, %key_id, path, "gateway admission queue is full"),
		GatewayLimitError::Unavailable | GatewayLimitError::Misconfigured => tracing::error!(%team_id, %key_id, path, %error, "gateway admission failed"),
	}
}

#[must_use]
/// Maps neutral admission failures to the enabled `OpenAI` namespace.
pub fn limit_error_response(error: GatewayLimitError) -> Response {
	match error {
		GatewayLimitError::Unavailable => server_error(StatusCode::SERVICE_UNAVAILABLE, "gateway.errors.limiter_unavailable", "gateway_limiter_unavailable"),
		GatewayLimitError::Overloaded => server_error(StatusCode::SERVICE_UNAVAILABLE, "gateway.errors.limiter_overloaded", "gateway_limiter_overloaded"),
		GatewayLimitError::Misconfigured => server_error(
			StatusCode::INTERNAL_SERVER_ERROR,
			"gateway.errors.limiter_misconfigured",
			"gateway_limiter_misconfigured",
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

fn server_error(status: StatusCode, key: &str, code: &str) -> Response {
	error_response(status, translation(key), "server_error", code)
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
		for (name, value) in [(CONCURRENT_LIMIT_HEADER, limit), (CONCURRENT_REMAINING_HEADER, remaining)] {
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
