use crate::types::{GatewayAuthContext, GatewayForm, GatewayFormError, GatewayInference, GatewayModelsQuery, GatewayRequestScope, GatewayRoute, JobState, MediaKind, RequestLogContext};
use crate::utils::gateway_limits;
use crate::routes::gateway::media::{run_form, run_json};
use crate::routes::gateway::{admit_inference, error_response, run_chat, run_models, run_responses, translation};
use axum::extract::{Extension, Query, Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use omniference::server::SkinAwareJson;
use omniference::skins::{OpenAIAudioSkin, OpenAIDecisionsSkin, OpenAIEmbeddingsSkin, OpenAIErrorHandler, OpenAIImagesSkin, OpenAIRerankSkin, SkinErrorHandler};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use serde_json::{Map, Value};
use std::sync::Arc;

const AUDIO_FILES: [&str; 1] = ["file"];
const IMAGE_FILES: [&str; 3] = ["image", "image[]", "mask"];

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
pub async fn list_models(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>, Query(query): Query<GatewayModelsQuery>) -> Response {
	let model_type = query.model_type.as_deref();
	gateway_limits::run_limited(&state, &context, GatewayRoute::MODELS, Ok(0), |_| run_models(&state, &context, model_type)).await
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
		inference.apply_preset_to_chat(&mut request);
		inference.chat_estimate(&context.policy, &mut request)
	};
	let admitted = match admit_inference(&state, scope, GatewayRoute::CHAT, &requested, estimate).await {
		Ok(admitted) => admitted,
		Err(response) => return *response,
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
		inference.apply_preset_to_responses(&mut request);
		inference.responses_estimate(&context.policy, &mut request)
	};
	let admitted = match admit_inference(&state, scope, GatewayRoute::RESPONSES, &requested, estimate).await {
		Ok(admitted) => admitted,
		Err(response) => return *response,
	};
	let response = run_responses(request, admitted.inference, admitted.run).await;
	gateway_limits::complete(&state, admitted.admission, response)
}

fn media_scope<'a>(state: &'a JobState, context: &'a GatewayAuthContext, log: RequestLogContext, route: GatewayRoute, streaming: bool) -> GatewayRequestScope<'a> {
	GatewayRequestScope {
		db: &state.db,
		context,
		log,
		usage_type: route.usage_type,
		streaming,
	}
}

/// Runs an authorized OpenAI-compatible embeddings request.
pub async fn embeddings(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let route = GatewayRoute::EMBEDDINGS;
	let scope = media_scope(&state, &context, log, route, false);
	run_json(&state, scope, (route, MediaKind::Embeddings), body, OpenAIEmbeddingsSkin::handle_embeddings).await
}

/// Runs an authorized rerank request (Jina, Cohere v1 and vLLM shape).
pub async fn rerank(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let route = GatewayRoute::RERANK;
	let scope = media_scope(&state, &context, log, route, false);
	run_json(&state, scope, (route, MediaKind::Rerank), body, OpenAIRerankSkin::handle_rerank).await
}

/// Runs an authorized OpenAI-compatible decisions request.
pub async fn decisions(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let route = GatewayRoute::DECISIONS;
	let scope = media_scope(&state, &context, log, route, false);
	run_json(&state, scope, (route, MediaKind::Decision), body, OpenAIDecisionsSkin::handle_decisions).await
}

/// Runs an authorized OpenAI-compatible image generation request.
pub async fn image_generations(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let route = GatewayRoute::IMAGE_GENERATIONS;
	let scope = media_scope(&state, &context, log, route, false);
	run_json(&state, scope, (route, MediaKind::Image { images: 0 }), body, OpenAIImagesSkin::handle_generations).await
}

/// Runs an authorized OpenAI-compatible text to speech request.
pub async fn speech(
	State(state): State<Arc<JobState>>,
	Extension(context): Extension<GatewayAuthContext>,
	Extension(log): Extension<RequestLogContext>,
	SkinAwareJson(body): SkinAwareJson<Map<String, Value>>,
) -> Response {
	let route = GatewayRoute::SPEECH;
	let streaming = body.get("stream_format").and_then(Value::as_str) == Some("sse");
	let scope = media_scope(&state, &context, log, route, streaming);
	run_json(&state, scope, (route, MediaKind::Speech), body, OpenAIAudioSkin::handle_speech).await
}

/// Runs an authorized OpenAI-compatible multipart image edit request.
pub async fn image_edits(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>, Extension(log): Extension<RequestLogContext>, request: Request) -> Response {
	let route = GatewayRoute::IMAGE_EDITS;
	let form = match GatewayForm::read(request).await {
		Ok(form) => form,
		Err(error) => return invalid_form(error),
	};
	let (images, _) = form.files(&IMAGE_FILES);
	let scope = media_scope(&state, &context, log, route, false);
	run_form(&state, scope, (route, MediaKind::Image { images }), (form, &IMAGE_FILES), OpenAIImagesSkin::handle_edits).await
}

/// Runs an authorized OpenAI-compatible multipart transcription request.
pub async fn transcriptions(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>, Extension(log): Extension<RequestLogContext>, request: Request) -> Response {
	transcribe(&state, &context, log, GatewayRoute::TRANSCRIPTIONS, request).await
}

/// Runs an authorized OpenAI-compatible multipart translation request.
pub async fn translations(State(state): State<Arc<JobState>>, Extension(context): Extension<GatewayAuthContext>, Extension(log): Extension<RequestLogContext>, request: Request) -> Response {
	transcribe(&state, &context, log, GatewayRoute::TRANSLATIONS, request).await
}

async fn transcribe(state: &JobState, context: &GatewayAuthContext, log: RequestLogContext, route: GatewayRoute, request: Request) -> Response {
	let form = match GatewayForm::read(request).await {
		Ok(form) => form,
		Err(error) => return invalid_form(error),
	};
	let (_, audio_bytes) = form.files(&AUDIO_FILES);
	let streaming = form.text("stream").is_some_and(|stream| stream.trim() == "true");
	let scope = media_scope(state, context, log, route, streaming);
	let kind = (route, MediaKind::Transcription { audio_bytes });
	if route.path == GatewayRoute::TRANSLATIONS.path {
		run_form(state, scope, kind, (form, &AUDIO_FILES), OpenAIAudioSkin::handle_translations).await
	} else {
		run_form(state, scope, kind, (form, &AUDIO_FILES), OpenAIAudioSkin::handle_transcriptions).await
	}
}

fn invalid_form(error: GatewayFormError) -> Response {
	error_response(error.status, error.message, "invalid_request_error", "invalid_request_body")
}
