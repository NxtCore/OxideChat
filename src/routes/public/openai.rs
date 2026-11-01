use crate::types::{GatewayAuthContext, GatewayInference, GatewayRequestScope, GatewayRoute, JobState, RequestLogContext};
use crate::utils::gateway_limits;
use crate::routes::gateway::{admit_inference, error_response, run_chat, run_models, run_responses, translation};
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::Response;
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIErrorHandler, SkinErrorHandler};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use std::sync::Arc;

/// Returns an OpenAI-compatible not-found response.
pub async fn not_found() -> Response {
	OpenAIErrorHandler.handle_not_found()
}

/// Returns an OpenAI-compatible method-not-allowed response.
pub async fn method_not_allowed() -> Response {
	OpenAIErrorHandler.handle_method_not_allowed()
}

/// Lists enabled models accessible to the authenticated gateway API key.
///
/// This route is not logged: a model listing runs no inference, so it has no model, tokens or cost
/// and would only fill the request log with non-inference rows.
pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>) -> Response {
	gateway_limits::run_limited(&state, &context, GatewayRoute::MODELS, Ok(0), |_| run_models(&state, &context)).await
}

/// Runs an authorized OpenAI-compatible chat completion request.
pub async fn chat_completions(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(mut request): SkinAwareJson<OpenAIChatRequest>,
) -> Response {
	let scope = GatewayRequestScope {
		db: &state.db,
		context: &context,
		log,
		usage_type: GatewayRoute::CHAT.usage_type,
		streaming: request.stream.unwrap_or(false),
	};
	let requested = request.model.clone();
	let estimate = |inference: &mut GatewayInference| {
		request.model.clone_from(&inference.engine_model_id);
		inference.chat_estimate(&context.policy, &mut request)
	};
	let admitted = match admit_inference(&state, scope, GatewayRoute::CHAT, &requested, estimate).await {
		Ok(admitted) => admitted,
		Err(response) => return response,
	};
	let response = run_chat(request, admitted.inference, admitted.run).await;
	gateway_limits::complete(&state, admitted.admission, response)
}

/// Runs an authorized OpenAI-compatible responses request.
pub async fn responses(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(mut request): SkinAwareJson<OpenAIResponsesRequestPayload>,
) -> Response {
	let scope = GatewayRequestScope {
		db: &state.db,
		context: &context,
		log,
		usage_type: GatewayRoute::RESPONSES.usage_type,
		streaming: request.stream.unwrap_or(false),
	};
	let Some(requested) = request.model.clone() else {
		return error_response(
			StatusCode::BAD_REQUEST,
			translation("gateway.errors.model_missing"),
			"invalid_request_error",
			"missing_required_parameter",
		);
	};
	let estimate = |inference: &mut GatewayInference| {
		request.model = Some(inference.engine_model_id.clone());
		inference.responses_estimate(&context.policy, &mut request)
	};
	let admitted = match admit_inference(&state, scope, GatewayRoute::RESPONSES, &requested, estimate).await {
		Ok(admitted) => admitted,
		Err(response) => return response,
	};
	let response = run_responses(request, admitted.inference, admitted.run).await;
	gateway_limits::complete(&state, admitted.admission, response)
}
