use super::{GatewayInference, GatewayLimitError, GatewayRatePolicy};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use serde_json::Value;

const EXTERNAL_INPUT_KEYS: [&str; 8] = [
	"image_url",
	"input_audio",
	"file_id",
	"file_url",
	"video_url",
	"previous_response_id",
	"conversation",
	"prompt",
];

impl GatewayInference {
	/// Estimates text conservatively and reserves the model context for external input.
	///
	/// # Errors
	/// Returns [`GatewayLimitError::Misconfigured`] if a required output or context bound is absent.
	pub fn estimate_tokens(&self, body: &[u8], output_max: Option<u64>, choices: u64) -> Result<u64, GatewayLimitError> {
		let output = output_max
			.or(self.max_output_tokens.map(u64::from))
			.filter(|value| *value > 0)
			.ok_or(GatewayLimitError::Misconfigured)?;
		let input = if has_external_input(body) {
			self.context_length.map(u64::from).ok_or(GatewayLimitError::Misconfigured)?
		} else {
			u64::try_from(body.len()).unwrap_or(u64::MAX).saturating_mul(2).saturating_add(1024)
		};
		Ok(input.saturating_add(output).saturating_mul(choices.max(1)))
	}

	/// Applies the output maximum to a Chat Completions request and returns its token reservation.
	///
	/// # Errors
	/// Returns a limit error if a conservative reservation cannot be computed.
	pub fn chat_tokens(&self, policy: &GatewayRatePolicy, request: &mut OpenAIChatRequest) -> Result<u64, GatewayLimitError> {
		if !policy.has_token_rules() {
			return Ok(0);
		}
		let output_max = request.max_completion_tokens.or(request.max_tokens).or(self.max_output_tokens);
		if request.max_completion_tokens.is_none() && request.max_tokens.is_none() {
			request.max_completion_tokens = output_max;
		}
		let body = serde_json::to_vec(&*request).map_err(|_| GatewayLimitError::Unavailable)?;
		self.estimate_tokens(&body, output_max.map(u64::from), u64::from(request.n.unwrap_or(1)))
	}

	/// Applies the output maximum to a Responses request and returns its token reservation.
	///
	/// # Errors
	/// Returns a limit error if a conservative reservation cannot be computed.
	pub fn responses_tokens(&self, policy: &GatewayRatePolicy, request: &mut OpenAIResponsesRequestPayload) -> Result<u64, GatewayLimitError> {
		if !policy.has_token_rules() {
			return Ok(0);
		}
		if request.max_output_tokens.is_none() {
			request.max_output_tokens = self.max_output_tokens.map(i64::from);
		}
		let output_max = request.max_output_tokens.and_then(|value| u64::try_from(value).ok());
		let body = serde_json::to_vec(&*request).map_err(|_| GatewayLimitError::Unavailable)?;
		self.estimate_tokens(&body, output_max, 1)
	}
}

fn has_external_input(body: &[u8]) -> bool {
	let Ok(text) = std::str::from_utf8(body) else {
		return false;
	};
	EXTERNAL_INPUT_KEYS.iter().any(|key| text.contains(key)) && serde_json::from_slice::<Value>(body).is_ok_and(|value| value_has_external_input(&value))
}

fn value_has_external_input(value: &Value) -> bool {
	match value {
		Value::Object(fields) => fields
			.iter()
			.any(|(key, value)| (EXTERNAL_INPUT_KEYS.contains(&key.as_str()) && !value.is_null()) || value_has_external_input(value)),
		Value::Array(values) => values.iter().any(value_has_external_input),
		_ => false,
	}
}
