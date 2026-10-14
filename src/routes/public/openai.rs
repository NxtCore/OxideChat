use crate::types::{GatewayAuthContext, GatewayModel, GatewayRoute, JobState};
use crate::utils::gateway_limits;
use crate::utils::openai_gateway::{error_response, model_access_error_response, run_chat, run_models, run_responses, translation};
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
pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>) -> Response {
	gateway_limits::run_limited(&state, &context, GatewayRoute::MODELS, Ok(0), |_| run_models(&state, &context)).await
}

/// Runs an authorized OpenAI-compatible chat completion request.
pub async fn chat_completions(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(mut request): SkinAwareJson<OpenAIChatRequest>,
) -> Response {
	let inference = match GatewayModel::authorize_inference(&state.db, &context, &request.model).await {
		Ok(inference) => inference,
		Err(error) => return model_access_error_response(error, &request.model),
	};
	let tokens = inference.chat_tokens(&context.policy, &mut request);
	gateway_limits::run_limited(&state, &context, GatewayRoute::CHAT, tokens, |id| run_chat(request, &context, inference, id)).await
}

/// Runs an authorized OpenAI-compatible responses request.
pub async fn responses(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(mut request): SkinAwareJson<OpenAIResponsesRequestPayload>,
) -> Response {
	let Some(model) = request.model.as_deref() else {
		return error_response(
			StatusCode::BAD_REQUEST,
			translation("gateway.errors.model_missing"),
			"invalid_request_error",
			"missing_required_parameter",
		);
	};
	let inference = match GatewayModel::authorize_inference(&state.db, &context, model).await {
		Ok(inference) => inference,
		Err(error) => return model_access_error_response(error, model),
	};
	let tokens = inference.responses_tokens(&context.policy, &mut request);
	gateway_limits::run_limited(&state, &context, GatewayRoute::RESPONSES, tokens, |id| run_responses(request, &context, inference, id)).await
}
