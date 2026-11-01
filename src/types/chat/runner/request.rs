use super::history::get_omni_messages;
use super::prepare::TurnError;
use crate::ai;
use crate::types::models::Model;
use crate::types::models_configs::ModelConfig;
use crate::types::{Message, StreamRequest, Tool, ToolFunction, User};
use omniference::Sampling;
use omniference::types::{ChatRequestIR, ContentPart, Message as OmniMessage, Role, ToolChoice, ToolSpec};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) struct RequestInputs<'a> {
	pub user: &'a User,
	pub chat_id: &'a Uuid,
	pub req: &'a StreamRequest,
	pub model: &'a Model,
	pub user_config: Option<&'a ModelConfig>,
	pub system_config: Option<&'a ModelConfig>,
}

/// Interpolate `{{variable}}` placeholders in a system prompt string.
///
/// Supported variables:
/// - `{{user_name}}` — the username of the authenticated user
/// - `{{user_email}}` — the email address of the authenticated user
/// - `{{date}}` — current UTC date (YYYY-MM-DD)
/// - `{{time}}` — current UTC time (HH:MM)
/// - `{{datetime}}` — current UTC date and time (YYYY-MM-DD HH:MM)
/// - `{{model_name}}` — display name of the model being used
/// - `{{model_id}}` — the model's identifier string
fn interpolate_system_prompt(template: &str, user: &User, model: &Model) -> String {
	let now = chrono::Utc::now();
	template
		.replace("{{user_name}}", &user.username)
		.replace("{{user_email}}", &user.email)
		.replace("{{date}}", &now.format("%Y-%m-%d").to_string())
		.replace("{{time}}", &now.format("%H:%M").to_string())
		.replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M").to_string())
		.replace("{{model_name}}", &model.display_name)
		.replace("{{model_id}}", &model.model_id)
}

/// Merge sampling options with priority order:
/// 1. Request-level (highest priority)
/// 2. `model_configs` (user preferences)
/// 3. models table (defaults)
fn merge_sampling_with_priority(request_sampling: Option<&Sampling>, model_config: Option<&ModelConfig>, model: &Model) -> Sampling {
	let config_sampling: Option<Sampling> = model_config.and_then(|mc| serde_json::from_value(mc.sampling.0.clone()).ok());

	macro_rules! priority_field {
		($field:ident) => {
			request_sampling
				.and_then(|s| s.$field.clone())
				.or_else(|| config_sampling.as_ref().and_then(|s| s.$field.clone()))
		};
	}

	Sampling {
		temperature: priority_field!(temperature),
		top_p: priority_field!(top_p),
		top_k: priority_field!(top_k),
		max_tokens: request_sampling
			.and_then(|s| s.max_tokens)
			.or_else(|| config_sampling.as_ref().and_then(|s| s.max_tokens))
			.or_else(|| model_config.and_then(|mc| mc.max_output_tokens.map(|t| t as u32)))
			.or_else(|| model.max_tokens.map(|tokens| tokens as u32)),
		presence_penalty: priority_field!(presence_penalty),
		frequency_penalty: priority_field!(frequency_penalty),
		stop: request_sampling
			.map(|s| s.stop.clone())
			.filter(|v| !v.is_empty())
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
/// 2. `model_configs` `extra_settings`
fn merge_reasoning_with_priority(
	request_effort: Option<&String>,
	request_budget: Option<u32>,
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

	let effort = request_effort.cloned().or(config_effort);
	let budget = request_budget.or(config_budget);

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

async fn base_request(db: &PgPool, inputs: &RequestInputs<'_>, messages: Vec<Message>) -> Result<ChatRequestIR, TurnError> {
	let req = inputs.req;
	let engine = ai::get();

	let Some(omni_model) = engine.get_model(&req.model_key).await else {
		let msg = format!("Model '{}' not found", req.model_key);
		return Err(TurnError::new("model_not_found", msg));
	};

	let Some(provider) = engine.get_provider(&omni_model.provider_name).await else {
		let msg = format!("Provider '{}' not found", omni_model.provider_name);
		return Err(TurnError::new("provider_not_found", msg));
	};

	let vision = omni_model.input_modalities.iter().any(|m| matches!(m, omniference::types::Modality::Image));
	let omni_messages = get_omni_messages(db, messages, vision).await;
	let mut ir = ChatRequestIR::default();
	ir.model.alias = omni_model.id.clone();
	ir.model.model_id = omni_model.id.clone();
	ir.model.provider = provider;
	ir.model.input_modalities = omni_model.input_modalities.clone();
	ir.model.output_modalities = omni_model.output_modalities.clone();
	ir.messages = omni_messages;
	ir.stream = true;
	ir.metadata.insert("user_id".to_string(), inputs.user.id.to_string());
	ir.metadata.insert("chat_id".to_string(), inputs.chat_id.to_string());
	ir.sampling = merge_sampling_with_priority(req.sampling.as_ref(), inputs.user_config, inputs.model);
	ir.reasoning = merge_reasoning_with_priority(req.reasoning_effort.as_ref(), req.reasoning_budget_tokens, inputs.system_config);
	Ok(ir)
}

fn apply_provider_routing(ir: &mut ChatRequestIR, req: &StreamRequest) {
	if crate::config::Config::get().enable_provider_selector()
		&& let Some(slug) = req.provider_slug.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
			&& ir.model.provider.endpoint.kind == omniference::types::ProviderKind::OpenRouter {
				ir.provider_routing = Some(match req.provider_routing_mode.as_deref() {
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
}

async fn apply_tools_and_system_prompt(db: &PgPool, inputs: &RequestInputs<'_>, ir: &mut ChatRequestIR) {
	let mut tool_system_prompts: Vec<String> = Vec::new();
	if let Some(ref enabled_tool_ids) = inputs.req.tools_enabled {
		let (specs, choice, prompts) = resolve_tools(db, inputs.user.id, enabled_tool_ids).await;
		if !specs.is_empty() {
			ir.tools = specs;
			ir.tool_choice = choice;
			tool_system_prompts = prompts;
		}
	}

	let mut system_segments: Vec<String> = Vec::new();
	let system_prompt_text = inputs.system_config.and_then(|mc| mc.system_prompt.as_deref()).unwrap_or("");
	if !system_prompt_text.is_empty() {
		system_segments.push(interpolate_system_prompt(system_prompt_text, inputs.user, inputs.model));
	}
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
	apply_provider_routing(&mut ir, inputs.req);
	apply_tools_and_system_prompt(db, inputs, &mut ir).await;
	Ok(ir)
}
