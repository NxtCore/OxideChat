use super::request::{RequestInputs, build_request};
use crate::types::models::Model;
use crate::types::models_configs::{ModelConfig, ModelConfigViewer};
use crate::types::{
	BudgetAdmission, BudgetError, BudgetSubject, Chat, ChatUsageGuard, CostDetails, JobState, Message, ReasoningDetails, RequestSettings, StreamData, StreamRequest,
	StreamingUserMessageCreate, TokenEstimate, UsageDetails, User,
};
use axum::response::{
	IntoResponse, Response,
	sse::{Event, KeepAlive, Sse},
};
use omniference::types::ChatRequestIR;
use sqlx::PgPool;
use std::{convert::Infallible, sync::Arc};
use uuid::Uuid;

const DEFAULT_OUTPUT_ESTIMATE: u64 = 4096;

/// A failure before the stream starts, delivered to the client as a single SSE error event.
#[derive(Debug)]
pub struct TurnError {
	code: &'static str,
	message: String,
}

impl TurnError {
	pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
		Self { code, message: message.into() }
	}
}

impl IntoResponse for TurnError {
	fn into_response(self) -> Response {
		let data = serde_json::to_string(&StreamData::Error {
			code: self.code.to_string(),
			message: self.message,
		})
		.unwrap_or_default();
		Sse::new(futures_util::stream::once(async move { Ok::<_, Infallible>(Event::default().data(data)) }))
			.keep_alive(KeepAlive::default())
			.into_response()
	}
}

/// Everything the stream needs once admission, persistence and request building succeeded.
pub struct PreparedTurn {
	pub(crate) chat_id: Uuid,
	pub(crate) model_id: Uuid,
	pub(crate) usage: ChatUsageGuard,
	pub(crate) user_id: Uuid,
	pub(crate) user_message: Option<Message>,
	pub(crate) ir: ChatRequestIR,
	pub(crate) assistant_parent_id: Option<Uuid>,
	pub(crate) assistant_fork_index: i32,
	pub(crate) reasoning_effort: Option<String>,
	pub(crate) reasoning_budget_tokens: Option<u32>,
	pub(crate) request_settings: RequestSettings,
}

fn internal(message: &'static str) -> impl FnOnce(sqlx::Error) -> TurnError {
	move |error| {
		tracing::error!(%error, "{message}");
		TurnError::new("internal_error", message)
	}
}

async fn prompt_bytes(db: &PgPool, chat_id: &Uuid, req: &StreamRequest, system_config: Option<&ModelConfig>) -> Result<u64, TurnError> {
	let history_bytes = Message::active_prompt_bytes(db, chat_id).await.map_err(internal("Failed to check budget status"))?;
	let system_bytes = system_config.and_then(|config| config.system_prompt.as_deref()).map_or(0, str::len);
	Ok(history_bytes.saturating_add(req.content.len() as u64).saturating_add(system_bytes as u64))
}

async fn admit(state: &JobState, user_id: Uuid, model: &Model, prompt_bytes: u64) -> Result<BudgetAdmission, TurnError> {
	let estimate = TokenEstimate {
		input: TokenEstimate::input_for_bytes(prompt_bytes),
		output: model.max_tokens.and_then(|tokens| u64::try_from(tokens).ok()).filter(|tokens| *tokens > 0).unwrap_or(DEFAULT_OUTPUT_ESTIMATE),
	};
	match state.budget_ledger.reserve_for_model(&state.db, BudgetSubject::user(user_id), &model.id, estimate).await {
		Ok(admission) => Ok(admission),
		Err(BudgetError::Exceeded) => Err(TurnError::new("budget_exceeded", "Budget exceeded for this model")),
		Err(error) => {
			tracing::error!(%error, "failed to check budget status");
			Err(TurnError::new("internal_error", "Failed to check budget status"))
		}
	}
}

async fn store_user_message(db: &PgPool, chat_id: &Uuid, model_id: &Uuid, req: &StreamRequest, request_settings: &RequestSettings) -> Result<Option<Message>, TurnError> {
	let content_parts_json = req.parts.as_ref().and_then(|parts| serde_json::to_value(parts).ok());
	let last_active_message_id = Message::last_active_id(db, chat_id).await.ok().flatten();

	if req.skip_user_message {
		return Ok(None);
	}
	let message = Message::create_streaming_user(
		db,
		StreamingUserMessageCreate {
			chat_id,
			content: &req.content,
			content_parts: content_parts_json.as_ref(),
			model_id,
			reasoning_details: &ReasoningDetails {
				effort: req.reasoning_effort.clone(),
				budget_tokens: req.reasoning_budget_tokens.map(|b| b as i32),
			},
			usage_details: &UsageDetails::default(),
			cost_details: &CostDetails::default(),
			request_settings,
			parent_id: last_active_message_id,
		},
	)
	.await
	.map_err(|error| {
		tracing::error!(%error, "failed to save user message");
		TurnError::new("save_failed", "Failed to save message")
	})?;
	Ok(Some(message))
}

async fn regenerate_fork(db: &PgPool, chat_id: &Uuid, regen_id: &str, messages: Vec<Message>) -> Result<(Option<Uuid>, i32, Vec<Message>), TurnError> {
	let regen_uuid = Uuid::parse_str(regen_id).map_err(|_| TurnError::new("invalid_request", "Invalid regenerate_from_message_id"))?;

	let original = Message::find_by_id_and_chat(db, &regen_uuid, chat_id)
		.await
		.map_err(internal("Failed to fetch original message"))?
		.ok_or_else(|| TurnError::new("not_found", "Original message not found"))?;

	let parent_id = original.parent_id;
	let next_fork_index = Message::next_assistant_fork_index(db, chat_id, parent_id).await.unwrap_or(1);
	let _ = Message::deactivate_active_assistant_forks(db, chat_id, parent_id).await;

	let filtered_messages: Vec<Message> = if let Some(pid) = parent_id {
		let parent_idx = messages.iter().position(|m| m.id == pid);
		match parent_idx {
			Some(idx) => messages.into_iter().take(idx + 1).collect(),
			None => messages,
		}
	} else {
		vec![]
	};

	Ok((parent_id, next_fork_index, filtered_messages))
}

async fn select_fork(db: &PgPool, chat_id: &Uuid, req: &StreamRequest, user_message: Option<&Message>) -> Result<(Option<Uuid>, i32, Vec<Message>), TurnError> {
	let messages = Message::list_active_by_chat(db, chat_id).await.map_err(internal("Failed to fetch messages"))?;

	if let Some(ref regen_id) = req.regenerate_from_message_id {
		regenerate_fork(db, chat_id, regen_id, messages).await
	} else {
		let parent = user_message.map(|m| m.id).or_else(|| messages.last().map(|m| m.id));
		Ok((parent, 1, messages))
	}
}

/// Checks access and budget, stores the user message, picks the fork and builds the model request.
///
/// # Errors
/// Returns a [`TurnError`] whose code and message are sent to the client as an SSE error event.
pub async fn prepare_turn(state: &Arc<JobState>, user: &User, chat_id: Uuid, req: &StreamRequest) -> Result<PreparedTurn, TurnError> {
	let db = &state.db;

	Chat::find_by_id_and_user(db, &chat_id, &user.id)
		.await
		.map_err(internal("Failed to fetch chat"))?
		.ok_or_else(|| TurnError::new("not_found", "Chat not found"))?;

	let model = Model::find_by_model_id(db, &req.model_key)
		.await
		.map_err(internal("Failed to fetch model"))?
		.ok_or_else(|| TurnError::new("not_found", "Model not found"))?;

	if !Model::can_user_use_model(db, &user.id, &model.id).await.map_err(internal("Failed to check model access"))? {
		return Err(TurnError::new("model_not_allowed", "Model not available for this user"));
	}

	let user_config = ModelConfig::find_for_user_by_stable_key(db, ModelConfigViewer { user_id: &user.id }, &req.model_key)
		.await
		.ok()
		.flatten();
	let system_config = ModelConfig::find_system_by_stable_key(db, &req.model_key).await.ok().flatten();

	let bytes = prompt_bytes(db, &chat_id, req, system_config.as_ref()).await?;
	let budget = admit(state, user.id, &model, bytes).await?;

	let request_settings = RequestSettings {
		model_key: Some(req.model_key.clone()),
		provider_slug: req.provider_slug.as_ref().map(|slug| slug.trim().to_string()).filter(|slug| !slug.is_empty()),
		provider_routing_mode: Some(req.provider_routing_mode.clone().unwrap_or_else(|| "prefer".to_string())),
		enabled_tools: req.tools_enabled.clone().unwrap_or_default(),
	};

	let user_message = store_user_message(db, &chat_id, &model.id, req, &request_settings).await?;
	let (assistant_parent_id, assistant_fork_index, messages) = select_fork(db, &chat_id, req, user_message.as_ref()).await?;

	let inputs = RequestInputs {
		user,
		chat_id: &chat_id,
		req,
		model: &model,
		user_config: user_config.as_ref(),
		system_config: system_config.as_ref(),
	};
	let ir = build_request(db, &inputs, messages).await?;

	Ok(PreparedTurn {
		chat_id,
		model_id: model.id,
		usage: ChatUsageGuard::new(Arc::clone(state), budget.reservation, user.id, model.id, model.provider_id),
		user_id: user.id,
		user_message,
		ir,
		assistant_parent_id,
		assistant_fork_index,
		reasoning_effort: req.reasoning_effort.clone(),
		reasoning_budget_tokens: req.reasoning_budget_tokens,
		request_settings,
	})
}
