use super::GatewayInference;
use crate::types::{Preset, PresetReasoning, PresetSampling};
use omniference::types::providers::openai::{
	OpenAIChatRequest, OpenAIMessage, OpenAIMessageContent, OpenAIReasoningEffort, OpenAIResponsesRequestPayload, OpenAIStop, Reasoning,
};
use uuid::Uuid;

/// The parts of a preset the gateway applies to a request; tools only run in web chat.
#[derive(Debug, Clone)]
pub struct GatewayPreset {
	pub id: Uuid,
	pub system_prompt: Option<String>,
	pub sampling: PresetSampling,
	pub reasoning: PresetReasoning,
}

impl From<Preset> for GatewayPreset {
	fn from(preset: Preset) -> Self {
		Self {
			id: preset.id,
			system_prompt: preset.system_prompt,
			sampling: preset.sampling.0,
			reasoning: preset.reasoning.0,
		}
	}
}

fn system_message(prompt: &str) -> OpenAIMessage {
	OpenAIMessage {
		role: "system".to_string(),
		content: Some(OpenAIMessageContent::Text(prompt.to_string())),
		name: None,
		tool_calls: None,
		tool_call_id: None,
		function_call: None,
		refusal: None,
		audio: None,
		extra: Default::default(),
	}
}

fn chat_effort(effort: Option<&str>) -> Option<OpenAIReasoningEffort> {
	serde_json::from_value(serde_json::Value::String(effort?.to_string())).ok()
}

impl GatewayInference {
	/// Puts the preset's prompt before the client's messages and fills only the sampling and reasoning fields the client left empty.
	pub fn apply_preset_to_chat(&self, request: &mut OpenAIChatRequest) {
		let Some(preset) = &self.preset else { return };
		if let Some(prompt) = &preset.system_prompt {
			request.messages.insert(0, system_message(prompt));
		}
		let sampling = &preset.sampling;
		request.temperature = request.temperature.or(sampling.temperature);
		request.top_p = request.top_p.or(sampling.top_p);
		if request.max_tokens.is_none() && request.max_completion_tokens.is_none() {
			request.max_completion_tokens = sampling.max_tokens;
		}
		request.presence_penalty = request.presence_penalty.or(sampling.presence_penalty);
		request.frequency_penalty = request.frequency_penalty.or(sampling.frequency_penalty);
		request.seed = request.seed.or(sampling.seed);
		request.logprobs = request.logprobs.or(sampling.logprobs);
		request.top_logprobs = request.top_logprobs.or(sampling.top_logprobs);
		request.parallel_tool_calls = request.parallel_tool_calls.or(sampling.parallel_tool_calls);
		if request.stop.is_none() {
			request.stop = sampling.stop.clone().filter(|stop| !stop.is_empty()).map(OpenAIStop::Many);
		}
		if let Some(top_k) = sampling.top_k {
			request.extra.entry("top_k".to_string()).or_insert_with(|| top_k.into());
		}
		if request.reasoning_effort.is_none() {
			request.reasoning_effort = chat_effort(preset.reasoning.effort.as_deref());
		}
	}

	/// Puts the preset's prompt before the client's instructions and fills only the sampling and reasoning fields the client left empty.
	pub fn apply_preset_to_responses(&self, request: &mut OpenAIResponsesRequestPayload) {
		let Some(preset) = &self.preset else { return };
		if let Some(prompt) = &preset.system_prompt {
			request.instructions = Some(match request.instructions.take().filter(|own| !own.trim().is_empty()) {
				Some(own) => format!("{prompt}\n\n{own}"),
				None => prompt.clone(),
			});
		}
		let sampling = &preset.sampling;
		request.temperature = request.temperature.or(sampling.temperature.map(f64::from));
		request.top_p = request.top_p.or(sampling.top_p.map(f64::from));
		request.max_output_tokens = request.max_output_tokens.or(sampling.max_tokens.map(i64::from));
		request.parallel_tool_calls = request.parallel_tool_calls.or(sampling.parallel_tool_calls);
		request.top_logprobs = request.top_logprobs.or(sampling.top_logprobs.and_then(|n| i32::try_from(n).ok()));
		if let Some(effort) = &preset.reasoning.effort {
			let reasoning = request.reasoning.get_or_insert_with(Reasoning::default);
			if reasoning.effort.is_none() {
				reasoning.effort = Some(effort.clone());
			}
		}
	}
}
