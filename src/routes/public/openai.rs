use crate::types::GatewayRouteClass;
use crate::types::{GatewayAuthContext, GatewayModel, JobState};
use crate::utils::gateway_limits;
use crate::utils::openai_gateway::{error_response, model_access_error_response, run_chat, run_responses, translation};
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIErrorHandler, SkinErrorHandler};
use omniference::types::providers::OpenAIModelsResponse;
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

/// Lists enabled models accessible to the authenticated gateway project.
pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>) -> Response {
	let admission = match gateway_limits::admit(&state, &context, GatewayRouteClass::Utility, 0, "/models").await {
		Ok(admission) => admission,
		Err(response) => return response,
	};
	let models = match GatewayModel::list_for_context(&state.db, &context).await {
		Ok(models) => models,
		Err(error) => {
			tracing::error!(%error, "failed to list gateway models");
			return gateway_limits::finish(
				admission,
				error_response(
					StatusCode::INTERNAL_SERVER_ERROR,
					translation("gateway.errors.list_models"),
					"server_error",
					"internal_error",
				),
			);
		}
	};
	gateway_limits::finish(
		admission,
		axum::Json(OpenAIModelsResponse {
			object: Some("list".to_string()),
			data: models,
		})
		.into_response(),
	)
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
	let tokens = match gateway_limits::chat_tokens(&mut request, &context, &inference) {
		Ok(tokens) => tokens,
		Err(response) => return response,
	};
	let admission = match gateway_limits::admit(&state, &context, GatewayRouteClass::Inference, tokens, "/chat/completions").await {
		Ok(admission) => admission,
		Err(response) => return response,
	};
	let response = run_chat(request, &context, inference, admission.reservation.id).await;
	gateway_limits::finish(admission, response)
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
	let tokens = match gateway_limits::responses_tokens(&mut request, &context, &inference) {
		Ok(tokens) => tokens,
		Err(response) => return response,
	};
	let admission = match gateway_limits::admit(&state, &context, GatewayRouteClass::Inference, tokens, "/responses").await {
		Ok(admission) => admission,
		Err(response) => return response,
	};
	let response = run_responses(request, &context, inference, admission.reservation.id).await;
	gateway_limits::finish(admission, response)
}
