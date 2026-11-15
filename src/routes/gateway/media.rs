//! Non-chat gateway inference: embeddings, rerank, decisions, images and audio.

use super::{AdmittedInference, admit_inference, error_response, run_skin, translation};
use crate::types::{GatewayForm, GatewayInference, GatewayRequestScope, GatewayRoute, JobState, MediaKind};
use crate::utils::gateway_limits;
use axum::extract::{Extension, State, multipart::Multipart, multipart::MultipartRejection};
use axum::http::StatusCode;
use axum::response::Response;
use omniference::server::SkinAwareJson;
use omniference::skins::{SkinContext, SkinRequestMetadata};
use serde_json::{Map, Value};
use std::future::Future;

/// Runs a JSON non-chat request through its Omniference skin handler.
pub async fn run_json<F, Fut>(state: &JobState, scope: GatewayRequestScope<'_>, (route, kind): (GatewayRoute, MediaKind), mut body: Map<String, Value>, handler: F) -> Response
where
	F: FnOnce(State<SkinContext>, Option<Extension<SkinRequestMetadata>>, SkinAwareJson<Map<String, Value>>) -> Fut,
	Fut: Future<Output = Response>,
{
	let admitted = match admit(state, scope, (route, kind), &mut body).await {
		Ok(admitted) => admitted,
		Err(response) => return *response,
	};
	let response = run_skin(admitted.inference, admitted.run, SkinAwareJson(body), handler).await;
	gateway_limits::complete(state, admitted.admission, response)
}

/// Runs a multipart non-chat request through its Omniference skin handler; `files` names the
/// parts that are uploads rather than text fields.
pub async fn run_form<F, Fut>(state: &JobState, scope: GatewayRequestScope<'_>, (route, kind): (GatewayRoute, MediaKind), (mut form, files): (GatewayForm, &[&str]), handler: F) -> Response
where
	F: FnOnce(State<SkinContext>, Option<Extension<SkinRequestMetadata>>, Result<Multipart, MultipartRejection>) -> Fut,
	Fut: Future<Output = Response>,
{
	let mut fields = form.fields(files);
	let admitted = match admit(state, scope, (route, kind), &mut fields).await {
		Ok(admitted) => admitted,
		Err(response) => return *response,
	};
	form.set_text("model", &admitted.inference.engine_model_id);
	let multipart = form.into_multipart().await;
	let response = run_skin(admitted.inference, admitted.run, multipart, handler).await;
	gateway_limits::complete(state, admitted.admission, response)
}

async fn admit<'a>(state: &JobState, scope: GatewayRequestScope<'a>, (route, kind): (GatewayRoute, MediaKind), fields: &mut Map<String, Value>) -> Result<AdmittedInference<'a>, Box<Response>> {
	let Some(requested) = fields.get("model").and_then(Value::as_str).map(str::to_owned) else {
		return Err(Box::new(error_response(
			StatusCode::BAD_REQUEST,
			translation("gateway.errors.model_missing"),
			"invalid_request_error",
			"missing_required_parameter",
		)));
	};
	let context = scope.context;
	let estimate = |inference: &mut GatewayInference| {
		fields.insert("model".to_owned(), Value::String(inference.engine_model_id.clone()));
		inference.media_estimate(&context.policy, kind, fields)
	};
	admit_inference(state, scope, route, &requested, estimate).await
}
