use crate::routes::gateway::{admit_inference, error_response, run_skin, translation};
use crate::types::{GatewayAuthContext, GatewayInference, GatewayModel, GatewayRequestScope, GatewayRoute, JobState, RequestLogContext};
use crate::utils::gateway_limits;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use omniference::server::SkinAwareJson;
use omniference::skins::{AnthropicErrorHandler, AnthropicMessagesSkin, SkinErrorHandler};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// Returns an Anthropic not-found response.
pub async fn not_found() -> Response {
	AnthropicErrorHandler.handle_not_found()
}

/// Returns an Anthropic method-not-allowed response.
pub async fn method_not_allowed() -> Response {
	AnthropicErrorHandler.handle_method_not_allowed()
}

/// Lists the chat models the authenticated key can call, in the Anthropic Models API shape.
///
/// Presets are left out because `/messages` does not apply them.
pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>) -> Response {
	gateway_limits::run_limited(&state, &context, GatewayRoute::MODELS, Ok(0), |_| async {
		let mut models = match GatewayModel::list_for_context(&state.db, &context).await {
			Ok(models) => models,
			Err(error) => {
				tracing::error!(%error, "failed to list gateway models");
				return error_response(StatusCode::INTERNAL_SERVER_ERROR, translation("gateway.errors.list_models"), "server_error", "internal_error");
			}
		};
		models.retain(|model| model.model_type.as_deref() == Some("chat") && !model.id.starts_with("presets/"));
		let data: Vec<Value> = models
			.iter()
			.map(|model| json!({"type": "model", "id": model.id, "display_name": model.id, "created_at": "1970-01-01T00:00:00Z"}))
			.collect();
		axum::Json(json!({
			"data": data,
			"has_more": false,
			"first_id": models.first().map(|model| &model.id),
			"last_id": models.last().map(|model| &model.id),
		}))
		.into_response()
	})
	.await
}

/// Runs an authorized Anthropic Messages request; `anthropic-version` and `anthropic-beta` are forwarded unchanged.
pub async fn messages(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	headers: HeaderMap,
	SkinAwareJson(mut body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let Some(requested) = body.get("model").and_then(Value::as_str).map(str::to_owned) else {
		return error_response(StatusCode::BAD_REQUEST, translation("gateway.errors.model_missing"), "invalid_request_error", "missing_required_parameter");
	};
	let route = GatewayRoute::MESSAGES;
	let scope = GatewayRequestScope {
		db: &state.db,
		context: &context,
		log,
		usage_type: route.usage_type,
		streaming: body.get("stream").and_then(Value::as_bool).unwrap_or(false),
	};
	let estimate = |inference: &mut GatewayInference| {
		body.insert("model".to_owned(), Value::String(inference.engine_model_id.clone()));
		inference.messages_estimate(&context.policy, &mut body)
	};
	let admitted = match admit_inference(&state, scope, route, &requested, estimate).await {
		Ok(admitted) => admitted,
		Err(response) => return *response,
	};
	let handler = |ctx, metadata, body| AnthropicMessagesSkin::handle_messages(ctx, metadata, headers, body);
	let response = run_skin(admitted.inference, admitted.run, SkinAwareJson(Value::Object(body)), handler).await;
	gateway_limits::complete(&state, admitted.admission, response)
}
