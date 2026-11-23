use super::agent::{sse, turn_events};
use super::prepare::{PreparedTurn, TurnError, admit, internal, output_estimate, prompt_bytes, require_knowledge_access, require_model_access, store_user_message};
use super::request::{RequestInputs, build_request};
use super::title::{TitleJob, generate_title};
use crate::i18n::I18n;
use crate::types::models::Model;
use crate::types::models_configs::ModelConfig;
use crate::types::{
	ARENA_REQUEST_TYPE, ArenaRound, Chat, ChatMessageResponse, CostDetails, JobState, Message, Preset, PresetArena, ReasoningDetails, RequestSettings, StreamData,
	StreamRequest, User, redact_usage,
};
use axum::response::sse::Event;
use futures_util::{Stream, StreamExt};
use rand::seq::SliceRandom;
use std::{convert::Infallible, sync::Arc, time::Instant};
use uuid::Uuid;

/// A prepared arena round: the stored user message and one turn per model, in the shuffled order they are shown.
pub struct ArenaTurn {
	round_id: Uuid,
	user_id: Uuid,
	chat_id: Uuid,
	options: PresetArena,
	user_message: Message,
	candidates: Vec<PreparedTurn>,
	/// The question to title the chat with, when it has no title yet.
	title_question: Option<String>,
}

fn unavailable(key: &str) -> TurnError {
	TurnError::new("not_found", I18n::get().translate(key, &None))
}

/// Checks every model of the arena and reserves budget for each, all or nothing, then stores the user message once and
/// builds one request per model with the arena's prompt, sampling and the level's reasoning.
///
/// Regenerating or continuing without a new user message is refused.
pub(super) async fn prepare_arena_turn(state: &Arc<JobState>, user: &User, chat: &Chat, req: &StreamRequest, preset: Preset) -> Result<ArenaTurn, TurnError> {
	if req.regenerate_from_message_id.is_some() || req.skip_user_message {
		return Err(TurnError::new("invalid_request", I18n::get().translate("chat.arena.errors.no_regenerate", &None)));
	}
	let db = &state.db;
	let mut levels = Vec::with_capacity(preset.levels.len());
	for level in preset.levels.iter() {
		let model = Model::find_by_id(db, &level.model_id)
			.await
			.map_err(internal("Failed to fetch model"))?
			.filter(|model| model.is_enabled)
			.ok_or_else(|| unavailable("presets.errors.base_model_unavailable"))?;
		require_model_access(db, &user.id, &model).await?;
		require_knowledge_access(db, &user.id, &chat.id, &model.id, None).await?;
		let level_preset = preset.clone().at_level(Some(&level.key)).ok_or_else(|| unavailable("presets.errors.level_not_found"))?;
		let config = ModelConfig::find_system_by_stable_key(db, &model.model_id).await.ok().flatten();
		let bytes = prompt_bytes(db, &chat.id, req, config.as_ref(), Some(&level_preset)).await?;
		let budget = admit(state, user.id, &model, bytes).await?;
		levels.push((model, level_preset, config, budget));
	}
	levels.shuffle(&mut rand::rng());
	if let Some(ids) = &req.knowledge_base_ids {
		crate::types::rag::KnowledgeBase::attach_to_chat(db, &user.id, &chat.id, ids)
			.await
			.map_err(internal("Failed to load knowledge bases"))?;
	}

	let settings = RequestSettings {
		model_key: Some(req.model_key.clone()),
		..RequestSettings::default()
	};
	let user_message = store_user_message(db, &chat.id, None, req, &settings)
		.await?
		.ok_or_else(|| TurnError::new("save_failed", "Failed to save message"))?;
	let round_id = ArenaRound::create(db, &preset.id, &chat.id, &user.id, &user_message.id)
		.await
		.map_err(internal("Failed to start the arena round"))?;
	let history = Message::list_active_by_chat(db, &chat.id).await.map_err(internal("Failed to fetch messages"))?;
	let arena_req = StreamRequest {
		reasoning_effort: None,
		reasoning_budget_tokens: None,
		tools_enabled: None,
		sampling: None,
		provider_slug: None,
		provider_routing_mode: None,
		..req.clone()
	};

	let mut candidates = Vec::with_capacity(levels.len());
	for (fork_index, (model, level_preset, config, budget)) in (1..).zip(levels) {
		let inputs = RequestInputs {
			user,
			chat_id: &chat.id,
			req: &arena_req,
			chat_sampling: &chat.sampling,
			model: &model,
			preset: Some(&level_preset),
			system_config: config.as_ref(),
			enabled_tools: &[],
			provider_slugs: &[],
			config_tools: false,
		};
		let mut ir = build_request(db, &inputs, history.clone()).await?;
		ir.metadata.insert("oxide_request_type".to_string(), ARENA_REQUEST_TYPE.to_string());
		candidates.push(PreparedTurn {
			chat_id: chat.id,
			model_id: model.id,
			reservation: budget.reservation,
			output_estimate: output_estimate(&model),
			user_id: user.id,
			user_message: None,
			ir,
			assistant_parent_id: Some(user_message.id),
			assistant_fork_index: fork_index,
			reasoning_effort: level_preset.reasoning.effort.clone(),
			reasoning_budget_tokens: level_preset.reasoning.budget_tokens,
			request_settings: RequestSettings {
				model_key: Some(model.model_id.clone()),
				..RequestSettings::default()
			},
			needs_title: false,
			active: false,
			arena_round_id: Some(round_id),
			knowledge_base_ids: Vec::new(),
		});
	}

	Ok(ArenaTurn {
		round_id,
		user_id: user.id,
		chat_id: chat.id,
		options: preset.arena.0,
		user_message,
		candidates,
		title_question: chat.title.is_none().then(|| req.content.clone()),
	})
}

/// Removes everything from a message that would tell which model wrote it.
fn redact(message: &mut ChatMessageResponse, show_reasoning: bool) {
	message.model_id = None;
	message.model_key = None;
	message.request_settings = RequestSettings::default();
	message.reasoning_details = ReasoningDetails::default();
	message.cost_details = CostDetails::default();
	redact_usage(&mut message.usage_details);
	if !show_reasoning {
		message.reasoning_content = None;
	}
}

/// Streams an arena round in one response: every model's turn runs at once, and each event is wrapped with the position
/// of its answer as soon as it arrives.
///
/// Answers stay blind: models, costs and timings are stripped, reasoning too unless the arena shows it. With streaming
/// turned off only the finished answers and errors are sent, together once the last model is done. Dropping the
/// response drops every upstream request; answers that were already saved keep the round open for a pick.
pub fn stream_arena(state: Arc<JobState>, turn: ArenaTurn, start_time: Instant) -> impl Stream<Item = Result<Event, Infallible>> {
	async_stream::stream! {
		let ArenaTurn { round_id, user_id, chat_id, options, user_message, candidates, title_question } = turn;
		yield sse(StreamData::ArenaStarted {
			round_id,
			candidates: candidates.len(),
			stream_responses: options.stream_responses,
			show_reasoning: options.show_reasoning,
		});
		let mut saved: ChatMessageResponse = user_message.into();
		redact(&mut saved, true);
		yield sse(StreamData::UserMessageSaved { message: saved });

		let models: Vec<Uuid> = candidates.iter().map(|candidate| candidate.model_id).collect();
		let streams = candidates
			.into_iter()
			.enumerate()
			.map(|(candidate, turn)| turn_events(Arc::clone(&state), turn, start_time).map(move |event| (candidate, event)).boxed());
		let mut merged = futures_util::stream::select_all(streams);
		let mut held = Vec::new();
		let mut answer = None;
		while let Some((candidate, event)) = merged.next().await {
			let event = match event {
				StreamData::Done { mut message } => {
					if answer.is_none() && !message.content.is_empty() {
						answer = Some((models[candidate], message.content.clone()));
					}
					redact(&mut message, options.show_reasoning);
					StreamData::Done { message }
				}
				event @ StreamData::Error { .. } => event,
				StreamData::Usage { .. } | StreamData::Title { .. } => continue,
				StreamData::ReasoningDelta { .. } if !options.show_reasoning => continue,
				_ if !options.stream_responses => continue,
				event => event,
			};
			let wrapped = StreamData::ArenaCandidate { candidate, event: Box::new(event) };
			if options.stream_responses {
				yield sse(wrapped);
			} else {
				held.push(wrapped);
			}
		}
		for event in held {
			yield sse(event);
		}
		yield sse(StreamData::ArenaDone { round_id });

		if let (Some(question), Some((chat_model_id, answer))) = (title_question, answer) {
			let job = TitleJob { state, user_id, chat_id, chat_model_id, question, answer };
			if let Ok(Some(title)) = tokio::spawn(generate_title(job)).await {
				yield sse(StreamData::Title { title });
			}
		}
	}
}
