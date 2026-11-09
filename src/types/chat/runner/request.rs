use super::history::get_omni_messages;
use super::prepare::TurnError;
use crate::ai;
use crate::types::models::Model;
use crate::types::models_configs::ModelConfig;
use crate::types::{ChatSampling, Message, PRESET_METADATA_KEY, Preset, PresetReasoning, PresetSampling, StreamRequest, Tool, ToolFunction, User};
use omniference::Sampling;
use omniference::types::{ChatRequestIR, ContentPart, Message as OmniMessage, Role, ToolChoice, ToolSpec};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) struct RequestInputs<'a> {
	pub user: &'a User,
	pub chat_id: &'a Uuid,
	pub req: &'a StreamRequest,
	pub chat_sampling: &'a ChatSampling,
	pub model: &'a Model,
	pub preset: Option<&'a Preset>,
	pub system_config: Option<&'a ModelConfig>,
	/// The tools this turn may call: the request's, or the preset's when it locks them.
	pub enabled_tools: &'a [String],
	/// The upstream provider the user picked, already checked against what they may use.
	pub provider_slug: Option<&'a str>,
}

/// Interpolate `{{variable}}` placeholders in a system prompt string.
///
/// Supported variables:
/// - `{{user_name}}` — the username of the authenticated user
/// - `{{user_email}}` — the email address of the authenticated user
/// - `{{date}}` — current UTC date (YYYY-MM-DD)
/// - `{{time}}` — current UTC time (HH:MM)
/// - `{{datetime}}` — current UTC date and time (YYYY-MM-DD HH:MM)
/// - `{{model_name}}` — display name of the model or preset being used
/// - `{{model_id}}` — the model's or preset's identifier string
fn interpolate_system_prompt(template: &str, user: &User, model_name: &str, model_id: &str) -> String {
	let now = chrono::Utc::now();
	template
		.replace("{{user_name}}", &user.username)
		.replace("{{user_email}}", &user.email)
		.replace("{{date}}", &now.format("%Y-%m-%d").to_string())
		.replace("{{time}}", &now.format("%H:%M").to_string())
		.replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M").to_string())
		.replace("{{model_name}}", model_name)
		.replace("{{model_id}}", model_id)
}

/// The sampling sources of one request, from highest to lowest priority.
pub(crate) struct SamplingLayers<'a> {
	/// Options sent with the request.
	pub request: Option<&'a Sampling>,
	/// Options the user saved on the chat.
	pub chat: &'a ChatSampling,
	/// Defaults of the preset the request was made with.
	pub preset: Option<&'a PresetSampling>,
	/// The admin's configuration of the model.
	pub config: Option<&'a ModelConfig>,
	/// The model's own limits.
	pub model: &'a Model,
}

/// Merge sampling options with priority order:
/// 1. Request-level (highest priority)
/// 2. Chat-level options the user saved on the chat
/// 3. The preset's defaults
/// 4. The admin's model configuration
/// 5. models table (defaults)
pub(crate) fn merge_sampling_with_priority(layers: &SamplingLayers<'_>) -> Sampling {
	let request_sampling = layers.request;
	let model_config = layers.config;
	let model = layers.model;
	let config_sampling: Option<Sampling> = model_config
		.and_then(|mc| serde_json::from_value::<PresetSampling>(mc.sampling.0.clone()).ok())
		.map(|sampling| sampling.to_sampling());
	let preset_sampling: Option<Sampling> = layers.preset.map(PresetSampling::to_sampling);
	let chat_sampling = layers.chat.to_sampling();

	macro_rules! priority_field {
		($field:ident) => {
			request_sampling
				.and_then(|s| s.$field.clone())
				.or_else(|| chat_sampling.$field.clone())
				.or_else(|| preset_sampling.as_ref().and_then(|s| s.$field.clone()))
				.or_else(|| config_sampling.as_ref().and_then(|s| s.$field.clone()))
		};
	}

	Sampling {
		temperature: priority_field!(temperature),
		top_p: priority_field!(top_p),
		top_k: priority_field!(top_k),
		max_tokens: request_sampling
			.and_then(|s| s.max_tokens)
			.or(chat_sampling.max_tokens)
			.or_else(|| preset_sampling.as_ref().and_then(|s| s.max_tokens))
			.or_else(|| config_sampling.as_ref().and_then(|s| s.max_tokens))
			.or_else(|| model_config.and_then(|mc| mc.max_output_tokens.map(|t| t as u32)))
			.or_else(|| model.max_tokens.map(|tokens| tokens as u32)),
		presence_penalty: priority_field!(presence_penalty),
		frequency_penalty: priority_field!(frequency_penalty),
		stop: request_sampling
			.map(|s| s.stop.clone())
			.filter(|v| !v.is_empty())
			.or_else(|| preset_sampling.as_ref().map(|s| s.stop.clone()).filter(|v| !v.is_empty()))
			.or_else(|| config_sampling.as_ref().map(|s| s.stop.clone()).filter(|v| !v.is_empty()))
			.unwrap_or_default(),
		parallel_tool_calls: priority_field!(parallel_tool_calls),
		seed: priority_field!(seed),
		logit_bias: priority_field!(logit_bias),
		logprobs: priority_field!(logprobs),
		top_logprobs: priority_field!(top_logprobs),
	}
}

/// Merge reasoning config with priority order:
/// 1. Request-level (highest priority)
/// 2. The preset's defaults
/// 3. `model_configs` `extra_settings`
fn merge_reasoning_with_priority(
	request_effort: Option<&String>,
	request_budget: Option<u32>,
	preset: Option<&PresetReasoning>,
	model_config: Option<&ModelConfig>,
) -> Option<omniference::types::ReasoningConfig> {
	let config_effort: Option<String> = model_config
		.and_then(|mc| mc.extra_settings.get("reasoning_effort"))
		.and_then(|v| v.as_str())
		.map(std::string::ToString::to_string);

	let config_budget: Option<u32> = model_config
		.and_then(|mc| mc.extra_settings.get("reasoning_budget_tokens"))
		.and_then(serde_json::Value::as_u64)
		.map(|n| n as u32);

	let effort = request_effort.cloned().or_else(|| preset.and_then(|p| p.effort.clone())).or(config_effort);
	let budget = request_budget.or_else(|| preset.and_then(|p| p.budget_tokens)).or(config_budget);

	if effort.is_some() || budget.is_some() {
		Some(omniference::types::ReasoningConfig {
			effort,
			budget_tokens: budget,
			summary: Some("detailed".to_string()),
		})
	} else {
		None
	}
}

async fn resolve_tools(db: &PgPool, user_id: Uuid, enabled_tool_ids: &[String]) -> (Vec<ToolSpec>, ToolChoice, Vec<String>) {
	if enabled_tool_ids.is_empty() {
		return (vec![], ToolChoice::None, vec![]);
	}

	let tool_uuids: Vec<Uuid> = enabled_tool_ids.iter().filter_map(|id| Uuid::parse_str(id).ok()).collect();
	if tool_uuids.is_empty() {
		return (vec![], ToolChoice::None, vec![]);
	}

	let tools = Tool::find_enabled_for_user(db, &tool_uuids, &user_id).await.unwrap_or_default();

	if tools.is_empty() {
		return (vec![], ToolChoice::None, vec![]);
	}

	let tool_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();
	let functions: Vec<ToolFunction> = ToolFunction::list_by_tool_ids(db, &tool_ids).await.unwrap_or_default();

	let mut functions_by_tool: HashMap<Uuid, Vec<ToolFunction>> = HashMap::new();
	for f in functions {
		functions_by_tool.entry(f.tool_id).or_default().push(f);
	}

	let mut tool_specs: Vec<ToolSpec> = vec![];
	let mut system_prompts: Vec<String> = vec![];
	for tool in tools {
		if let Some(prompt) = tool.system_prompt.as_ref().map(|p| p.trim()).filter(|p| !p.is_empty()) {
			let label = if tool.display_name.trim().is_empty() {
				tool.name.as_str()
			} else {
				tool.display_name.trim()
			};
			system_prompts.push(format!("Instructions for the \"{label}\" tool:\n{prompt}"));
		}
		let funcs = functions_by_tool.get(&tool.id).map(std::vec::Vec::as_slice).unwrap_or(&[]);
		tool_specs.extend(tool.to_tool_specs(funcs));
	}

	if tool_specs.is_empty() {
		(vec![], ToolChoice::None, system_prompts)
	} else {
		(tool_specs, ToolChoice::Auto, system_prompts)
	}
}

/// Starts a streaming request for a model key with the model and provider filled in.
pub(super) async fn model_request(model_key: &str) -> Result<(ChatRequestIR, bool), TurnError> {
	let engine = ai::get();

	let Some(omni_model) = engine.get_model(model_key).await else {
		let msg = format!("Model '{model_key}' not found");
		return Err(TurnError::new("model_not_found", msg));
	};

	let Some(provider) = engine.get_provider(&omni_model.provider_name).await else {
		let msg = format!("Provider '{}' not found", omni_model.provider_name);
		return Err(TurnError::new("provider_not_found", msg));
	};

	let vision = omni_model.input_modalities.iter().any(|m| matches!(m, omniference::types::Modality::Image));
	let mut ir = ChatRequestIR::default();
	ir.model.alias = omni_model.id.clone();
	let provider_prefix = format!("{}/", omni_model.provider_name.to_lowercase());
	ir.model.model_id = omni_model.id.strip_prefix(&provider_prefix).unwrap_or(&omni_model.id).to_string();
	ir.model.provider = provider;
	ir.model.input_modalities = omni_model.input_modalities;
	ir.model.output_modalities = omni_model.output_modalities;
	ir.stream = true;
	Ok((ir, vision))
}

async fn base_request(db: &PgPool, inputs: &RequestInputs<'_>, messages: Vec<Message>) -> Result<ChatRequestIR, TurnError> {
	let req = inputs.req;
	let (mut ir, vision) = model_request(&inputs.model.model_id).await?;
	ir.messages = get_omni_messages(db, messages, vision).await;
	ir.metadata.insert("user_id".to_string(), inputs.user.id.to_string());
	ir.metadata.insert("chat_id".to_string(), inputs.chat_id.to_string());
	ir.metadata.insert("oxide_user_id".to_string(), inputs.user.id.to_string());
	ir.metadata.insert("oxide_model_id".to_string(), inputs.model.id.to_string());
	ir.metadata.insert("oxide_request_type".to_string(), "chat".to_string());
	if let Some(preset) = inputs.preset {
		ir.metadata.insert(PRESET_METADATA_KEY.to_string(), preset.id.to_string());
	}
	ir.sampling = merge_sampling_with_priority(&SamplingLayers {
		request: req.sampling.as_ref(),
		chat: inputs.chat_sampling,
		preset: inputs.preset.map(|preset| &preset.sampling.0),
		config: inputs.system_config,
		model: inputs.model,
	});
	ir.extra_body.clone_from(&inputs.chat_sampling.custom);
	ir.reasoning = merge_reasoning_with_priority(
		req.reasoning_effort.as_ref(),
		req.reasoning_budget_tokens,
		inputs.preset.map(|preset| &preset.reasoning.0),
		inputs.system_config,
	);
	Ok(ir)
}

fn apply_provider_routing(ir: &mut ChatRequestIR, inputs: &RequestInputs<'_>) {
	let Some(slug) = inputs.provider_slug else {
		return;
	};
	ir.provider_routing = Some(match inputs.req.provider_routing_mode.as_deref() {
		Some("lock") => omniference::types::ProviderRouting {
			only: Some(vec![slug.to_string()]),
			allow_fallbacks: Some(false),
			..Default::default()
		},
		_ => omniference::types::ProviderRouting {
			order: Some(vec![slug.to_string()]),
			..Default::default()
		},
	});
}

async fn apply_tools_and_system_prompt(db: &PgPool, inputs: &RequestInputs<'_>, ir: &mut ChatRequestIR) {
	let mut tool_system_prompts: Vec<String> = Vec::new();
	let (specs, choice, prompts) = resolve_tools(db, inputs.user.id, inputs.enabled_tools).await;
	if !specs.is_empty() {
		ir.tools = specs;
		ir.tool_choice = choice;
		tool_system_prompts = prompts;
	}

	let (model_name, model_id) = match inputs.preset {
		Some(preset) => (preset.name.clone(), preset.key()),
		None => (inputs.model.display_name.clone(), inputs.model.model_id.clone()),
	};
	let prompts = [
		inputs.system_config.and_then(|mc| mc.system_prompt.as_deref()),
		inputs.preset.and_then(|preset| preset.system_prompt.as_deref()),
	];
	let mut system_segments: Vec<String> = prompts
		.into_iter()
		.flatten()
		.filter(|prompt| !prompt.trim().is_empty())
		.map(|prompt| interpolate_system_prompt(prompt, inputs.user, &model_name, &model_id))
		.collect();
	system_segments.extend(tool_system_prompts);
	if !system_segments.is_empty() {
		ir.messages.insert(
			0,
			OmniMessage {
				role: Role::System,
				parts: vec![ContentPart::Text(system_segments.join("\n\n"))],
				name: None,
			},
		);
	}
}

pub(super) async fn build_request(db: &PgPool, inputs: &RequestInputs<'_>, messages: Vec<Message>) -> Result<ChatRequestIR, TurnError> {
	let mut ir = base_request(db, inputs, messages).await?;
	apply_provider_routing(&mut ir, inputs);
	apply_tools_and_system_prompt(db, inputs, &mut ir).await;
	Ok(ir)
}
