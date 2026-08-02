use crate::types::models::ModelPricing;
use crate::types::{Budget, GatewayAuthContext, GatewayModel, JobState};
use crate::utils::openai_gateway::{error_response, run_chat, run_responses};
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIErrorHandler, SkinErrorHandler, SkinRequestMetadata};
use omniference::types::providers::OpenAIModelsResponse;
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use std::sync::Arc;
use uuid::Uuid;

pub async fn not_found() -> Response {
	OpenAIErrorHandler.handle_not_found()
}

pub async fn method_not_allowed() -> Response {
	OpenAIErrorHandler.handle_method_not_allowed()
}

pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>) -> Response {
	let models = match GatewayModel::list_for_context(&state.db, &context).await {
		Ok(models) => models,
		Err(error) => {
			tracing::error!(%error, "failed to list gateway models");
			return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list models", "server_error", "internal_error");
		}
	};
	axum::Json(OpenAIModelsResponse {
		object: Some("list".to_string()),
		data: models,
	})
	.into_response()
}

pub async fn chat_completions(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(request): SkinAwareJson<OpenAIChatRequest>,
) -> Response {
	let model_id = match resolve_model_access(&state, &context, &request.model).await {
		Ok(model_id) => model_id,
		Err(response) => return response,
	};
	run_chat(request, request_metadata(&context, model_id)).await
}

pub async fn responses(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	SkinAwareJson(request): SkinAwareJson<OpenAIResponsesRequestPayload>,
) -> Response {
	let Some(model) = request.model.as_deref() else {
		return error_response(
			StatusCode::BAD_REQUEST,
			"Missing required parameter: 'model'.",
			"invalid_request_error",
			"missing_required_parameter",
		);
	};
	let model_id = match resolve_model_access(&state, &context, model).await {
		Ok(model_id) => model_id,
		Err(response) => return response,
	};
	run_responses(request, request_metadata(&context, model_id)).await
}

async fn resolve_model_access(state: &JobState, context: &GatewayAuthContext, model_id: &str) -> Result<Uuid, Response> {
	match GatewayModel::resolve_accessible(&state.db, context, model_id).await {
		Ok(Some(model_id)) => match budget_allows_model(state, &context.user_id, &model_id).await {
			Ok(true) => Ok(model_id),
			Ok(false) => Err(error_response(
				StatusCode::TOO_MANY_REQUESTS,
				"Budget exceeded for this model",
				"insufficient_quota",
				"budget_exceeded",
			)),
			Err(error) => {
				tracing::error!(%error, %model_id, "failed to evaluate gateway budget policy");
				Err(error_response(
					StatusCode::INTERNAL_SERVER_ERROR,
					"Failed to check budget status",
					"server_error",
					"internal_error",
				))
			}
		},
		Ok(None) => Err(OpenAIErrorHandler.handle_model_not_found(model_id)),
		Err(error) => {
			tracing::error!(%error, model_id, "failed to evaluate gateway model policy");
			Err(error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				"Failed to resolve model",
				"server_error",
				"internal_error",
			))
		}
	}
}

async fn budget_allows_model(state: &JobState, user_id: &Uuid, model_id: &Uuid) -> Result<bool, sqlx::Error> {
	if ModelPricing::is_free(&state.db, model_id).await? {
		return Ok(true);
	}
	let status = Budget::status_for_user(&state.db, user_id).await?;
	Ok(!status.blocked_model_ids.contains(model_id))
}

fn request_metadata(context: &GatewayAuthContext, model_id: Uuid) -> SkinRequestMetadata {
	let mut metadata = std::collections::BTreeMap::new();
	metadata.insert("oxide_user_id".to_string(), context.user_id.to_string());
	metadata.insert("oxide_project_id".to_string(), context.project_id.to_string());
	metadata.insert("oxide_api_key_id".to_string(), context.key_id.to_string());
	metadata.insert("oxide_model_id".to_string(), model_id.to_string());
	if let Some(team_id) = context.team_id {
		metadata.insert("oxide_team_id".to_string(), team_id.to_string());
	}
	SkinRequestMetadata(metadata)
}
