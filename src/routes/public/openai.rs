use crate::types::{GatewayAuthContext, GatewayModel, GatewayRequestScope, GatewayRoute, GatewayRun, JobState, RequestLogContext};
use crate::utils::gateway_limits;
use crate::utils::openai_gateway::{error_response, reject_limited, reject_model_access, run_chat, run_models, run_responses, translation};
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
	let mut inference = match GatewayModel::resolve_inference(&state.db, &context, &request.model).await {
		Ok(inference) => inference,
		Err(error) => return reject_model_access(&scope, None, error, &request.model).await,
	};
	let requested_model = std::mem::replace(&mut request.model, inference.engine_model_id.clone());
	let estimate = inference.chat_estimate(&context.policy, &mut request);
	if let Err(error) = GatewayModel::reserve_budget(&state.db, &state.budget_ledger, &context, &mut inference, estimate.tokens()).await {
		return reject_model_access(&scope, Some(&inference), error, &requested_model).await;
	}
	let admission = match gateway_limits::admit(&state, &context, GatewayRoute::CHAT, estimate.limiter_tokens()).await {
		Ok(admission) => admission,
		Err(error) => return reject_limited(&scope, &inference, error).await,
	};
	let run = GatewayRun {
		scope,
		reservation_id: admission.reservation.id,
	};
	let response = run_chat(request, inference, run).await;
	gateway_limits::complete(&state, admission, response)
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
	let Some(model) = request.model.as_deref() else {
		return error_response(
			StatusCode::BAD_REQUEST,
			translation("gateway.errors.model_missing"),
			"invalid_request_error",
			"missing_required_parameter",
		);
	};
	let mut inference = match GatewayModel::resolve_inference(&state.db, &context, model).await {
		Ok(inference) => inference,
		Err(error) => return reject_model_access(&scope, None, error, model).await,
	};
	let model = model.to_owned();
	request.model = Some(inference.engine_model_id.clone());
	let estimate = inference.responses_estimate(&context.policy, &mut request);
	if let Err(error) = GatewayModel::reserve_budget(&state.db, &state.budget_ledger, &context, &mut inference, estimate.tokens()).await {
		return reject_model_access(&scope, Some(&inference), error, &model).await;
	}
	let admission = match gateway_limits::admit(&state, &context, GatewayRoute::RESPONSES, estimate.limiter_tokens()).await {
		Ok(admission) => admission,
		Err(error) => return reject_limited(&scope, &inference, error).await,
	};
	let run = GatewayRun {
		scope,
		reservation_id: admission.reservation.id,
	};
	let response = run_responses(request, inference, run).await;
	gateway_limits::complete(&state, admission, response)
}
