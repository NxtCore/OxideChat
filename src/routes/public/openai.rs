use crate::types::models::Model;
use crate::types::{JobState, OpenAiModel, OpenAiModelsResponse};
use crate::utils::openai_gateway::{add_gateway_headers, authenticate, error_response, parse_json, run_chat, run_responses};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use std::sync::Arc;
use uuid::Uuid;

pub async fn not_found() -> Response {
	add_gateway_headers(
		error_response(StatusCode::NOT_FOUND, "The requested resource was not found", "not_found_error", "not_found"),
		None,
		Uuid::new_v4(),
	)
}

pub async fn method_not_allowed() -> Response {
	add_gateway_headers(
		error_response(
			StatusCode::METHOD_NOT_ALLOWED,
			"Invalid HTTP method for this endpoint",
			"invalid_request_error",
			"method_not_allowed",
		),
		None,
		Uuid::new_v4(),
	)
}

pub async fn list_models(State(state): State<Arc<JobState>>, headers: HeaderMap) -> impl IntoResponse {
	let request_id = Uuid::new_v4();
	let context = match authenticate(&state.db, &headers, "inference:read").await {
		Ok(context) => context,
		Err(response) => return add_gateway_headers(response, None, request_id),
	};
	let models = match OpenAiModel::list_for_user(&state.db, &context.user_id).await {
		Ok(models) => models,
		Err(error) => {
			tracing::error!(%error, "failed to list gateway models");
			return add_gateway_headers(
				error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list models", "server_error", "internal_error"),
				Some(&context),
				request_id,
			);
		}
	};
	add_gateway_headers(
		axum::Json(OpenAiModelsResponse {
			object: "list".to_string(),
			data: models,
		})
		.into_response(),
		Some(&context),
		request_id,
	)
}

pub async fn chat_completions(State(state): State<Arc<JobState>>, headers: HeaderMap, body: Bytes) -> Response {
	let request_id = Uuid::new_v4();
	let context = match authenticate(&state.db, &headers, "inference:write").await {
		Ok(context) => context,
		Err(response) => return add_gateway_headers(response, None, request_id),
	};
	let request: OpenAIChatRequest = match parse_json(&body) {
		Ok(request) => request,
		Err(message) => {
			return add_gateway_headers(
				error_response(StatusCode::BAD_REQUEST, message, "invalid_request_error", "invalid_request_body"),
				Some(&context),
				request_id,
			);
		}
	};
	if let Some(response) = validate_model_access(&state, &context.user_id, &request.model).await {
		return add_gateway_headers(response, Some(&context), request_id);
	}
	let response = run_chat(request).await;
	add_gateway_headers(response, Some(&context), request_id)
}

pub async fn responses(State(state): State<Arc<JobState>>, headers: HeaderMap, body: Bytes) -> Response {
	let request_id = Uuid::new_v4();
	let context = match authenticate(&state.db, &headers, "inference:write").await {
		Ok(context) => context,
		Err(response) => return add_gateway_headers(response, None, request_id),
	};
	let request: OpenAIResponsesRequestPayload = match parse_json(&body) {
		Ok(request) => request,
		Err(message) => {
			return add_gateway_headers(
				error_response(StatusCode::BAD_REQUEST, message, "invalid_request_error", "invalid_request_body"),
				Some(&context),
				request_id,
			);
		}
	};
	let Some(model) = request.model.as_deref() else {
		return add_gateway_headers(
			error_response(
				StatusCode::BAD_REQUEST,
				"Missing required parameter: 'model'.",
				"invalid_request_error",
				"missing_required_parameter",
			),
			Some(&context),
			request_id,
		);
	};
	if let Some(response) = validate_model_access(&state, &context.user_id, model).await {
		return add_gateway_headers(response, Some(&context), request_id);
	}
	let response = run_responses(request).await;
	add_gateway_headers(response, Some(&context), request_id)
}

async fn validate_model_access(state: &JobState, user_id: &Uuid, model_id: &str) -> Option<Response> {
	let model = match Model::find_by_model_id(&state.db, model_id).await {
		Ok(Some(model)) if model.is_enabled => model,
		Ok(_) => return Some(model_not_found(model_id)),
		Err(error) => {
			tracing::error!(%error, model_id, "failed to resolve gateway model");
			return Some(error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				"Failed to resolve model",
				"server_error",
				"internal_error",
			));
		}
	};
	match Model::can_user_use_model(&state.db, user_id, &model.id).await {
		Ok(true) => None,
		Ok(false) => Some(model_not_found(model_id)),
		Err(error) => {
			tracing::error!(%error, model_id, "failed to evaluate gateway model policy");
			Some(error_response(
				StatusCode::INTERNAL_SERVER_ERROR,
				"Failed to resolve model",
				"server_error",
				"internal_error",
			))
		}
	}
}

fn model_not_found(model_id: &str) -> Response {
	error_response(
		StatusCode::NOT_FOUND,
		format!("Model '{model_id}' not found"),
		"invalid_request_error",
		"model_not_found",
	)
}
