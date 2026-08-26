use crate::types::{GatewayAuthContext, GatewayModel, JobState};
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
	let models = match GatewayModel::list_for_context(&state.db, &context).await {
		Ok(models) => models,
		Err(error) => {
			tracing::error!(%error, "failed to list gateway models");
			return error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				translation("gateway.errors.list_models"),
				"server_error",
				"internal_error",
			);
		}
	};
	axum::Json(OpenAIModelsResponse {
		object: Some("list".to_string()),
		data: models,
	})
	.into_response()
}

/// Runs an authorized OpenAI-compatible chat completion request.
pub async fn chat_completions(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(request): SkinAwareJson<OpenAIChatRequest>,
) -> Response {
	let inference = match GatewayModel::authorize_inference(&state.db, &context, &request.model).await {
		Ok(inference) => inference,
		Err(error) => return model_access_error_response(error, &request.model),
	};
	run_chat(request, &context, inference).await
}

/// Runs an authorized OpenAI-compatible responses request.
pub async fn responses(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(request): SkinAwareJson<OpenAIResponsesRequestPayload>,
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
	run_responses(request, &context, inference).await
}
