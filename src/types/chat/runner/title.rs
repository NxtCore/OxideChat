use super::request::model_request;
use crate::ai;
use crate::config::Config;
use crate::types::models::Model;
use crate::types::{BUDGET_RESERVATION_METADATA_KEY, BudgetAdmission, BudgetSubject, Chat, JobState, TokenEstimate};
use omniference::types::{ContentPart, Message as OmniMessage, Role};
use std::sync::Arc;
use uuid::Uuid;

const TITLE_PROMPT: &str =
	"Write a short title of at most six words for this conversation, in the language of the user's message. Reply with the title only, without quotes.";
const MAX_INPUT_CHARS: usize = 2000;
const MAX_TITLE_CHARS: usize = 80;
const TITLE_OUTPUT_TOKENS: u32 = 64;

/// The first exchange of a chat that still has no title.
pub(crate) struct TitleJob {
	pub(crate) state: Arc<JobState>,
	pub(crate) user_id: Uuid,
	pub(crate) chat_id: Uuid,
	pub(crate) chat_model_id: Uuid,
	pub(crate) question: String,
	pub(crate) answer: String,
}

/// Turns a model reply into a title: its first line without quotes or markdown, `None` when empty.
pub(crate) fn clean_title(raw: &str) -> Option<String> {
	let line = raw.lines().map(str::trim).find(|line| !line.is_empty())?;
	let title = line
		.trim_start_matches(['#', '*', ' '])
		.trim_matches(['"', '\'', '*', '`', ' '])
		.trim_end_matches('.');
	let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
	let title = title.trim();
	(!title.is_empty()).then(|| title.to_string())
}

fn clip(text: &str) -> String {
	text.chars().take(MAX_INPUT_CHARS).collect()
}

fn title_messages(conversation: String) -> Vec<OmniMessage> {
	vec![
		OmniMessage {
			role: Role::System,
			parts: vec![ContentPart::Text(TITLE_PROMPT.to_string())],
			name: None,
		},
		OmniMessage {
			role: Role::User,
			parts: vec![ContentPart::Text(conversation)],
			name: None,
		},
	]
}

async fn reserve(job: &TitleJob, model_id: &Uuid, conversation: &str) -> Option<BudgetAdmission> {
	let estimate = TokenEstimate {
		input: TokenEstimate::input_for_bytes((TITLE_PROMPT.len() + conversation.len()) as u64),
		output: u64::from(TITLE_OUTPUT_TOKENS),
	};
	match job
		.state
		.budget_ledger
		.reserve_for_model(&job.state.db, BudgetSubject::user(job.user_id), model_id, estimate)
		.await
	{
		Ok(admission) => Some(admission),
		Err(error) => {
			tracing::debug!(%error, chat_id = %job.chat_id, "skipping title generation");
			None
		}
	}
}

fn usage_metadata(user_id: &Uuid, model_id: &Uuid, admission: &BudgetAdmission) -> Vec<(String, String)> {
	let mut metadata = vec![
		("oxide_user_id".to_string(), user_id.to_string()),
		("oxide_model_id".to_string(), model_id.to_string()),
		("oxide_request_type".to_string(), "title".to_string()),
	];
	metadata.extend(
		admission
			.reservation
			.as_ref()
			.map(|reservation| (BUDGET_RESERVATION_METADATA_KEY.to_string(), reservation.id.to_string())),
	);
	metadata
}

async fn save(job: &TitleJob, raw: &str) -> Option<String> {
	let title = clean_title(raw)?;
	match Chat::set_generated_title(&job.state.db, &job.chat_id, &title).await {
		Ok(saved) => saved.then_some(title),
		Err(error) => {
			tracing::error!(%error, chat_id = %job.chat_id, "failed to save generated title");
			None
		}
	}
}

/// Asks the configured title model, billed to the user's budget, for a title and stores it.
/// Returns the new title, or `None` when the setting is off, the budget is spent, or anything fails.
pub(crate) async fn generate_title(job: TitleJob) -> Option<String> {
	let setting = Config::get().title_generation();
	if !setting.enabled {
		return None;
	}
	let model_id = setting.model_id.unwrap_or(job.chat_model_id);
	let model = Model::find_by_id(&job.state.db, &model_id).await.ok().flatten()?;
	let (mut ir, _) = model_request(&model.model_id).await.ok()?;

	let conversation = format!("User: {}\n\nAssistant: {}", clip(&job.question), clip(&job.answer));
	let admission = reserve(&job, &model.id, &conversation).await?;
	ir.messages = title_messages(conversation);
	ir.sampling.max_tokens = Some(TITLE_OUTPUT_TOKENS);
	ir.metadata.extend(usage_metadata(&job.user_id, &model.id, &admission));

	let raw = ai::get()
		.chat_complete(ir)
		.await
		.inspect_err(|error| tracing::warn!(%error, chat_id = %job.chat_id, "title generation failed"))
		.ok()?;
	save(&job, &raw).await
}

/// The text of the first user message in a request's history.
pub(crate) fn first_user_text(messages: &[OmniMessage]) -> String {
	let Some(message) = messages.iter().find(|message| matches!(message.role, Role::User)) else {
		return String::new();
	};
	message
		.parts
		.iter()
		.filter_map(|part| match part {
			ContentPart::Text(text) => Some(text.as_str()),
			_ => None,
		})
		.collect::<Vec<_>>()
		.join("\n")
}
