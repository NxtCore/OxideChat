use super::{GatewayInference, GatewayLimitError, GatewayRatePolicy};
use crate::types::TokenEstimate;
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

/// Token bounds of one request and whether the limiter must reserve them.
pub struct GatewayEstimate {
	result: Result<TokenEstimate, GatewayLimitError>,
	limited: bool,
}

impl GatewayEstimate {
	#[must_use]
	/// Bounds used to price the request; an unbounded request is priced at zero.
	pub fn tokens(&self) -> TokenEstimate {
		self.result.as_ref().copied().unwrap_or_default()
	}

	/// Tokens for the limiter, which only reserves them when token rules apply.
	///
	/// # Errors
	/// Returns the estimation failure when token rules apply and no bound could be computed.
	pub fn limiter_tokens(self) -> Result<u64, GatewayLimitError> {
		match self.result {
			Ok(tokens) if self.limited => Ok(tokens.total()),
			Err(error) if self.limited => Err(error),
			_ => Ok(0),
		}
	}
}

impl GatewayInference {
	/// Estimates text conservatively and reserves the model context for external input.
	///
	/// # Errors
	/// Returns [`GatewayLimitError::Misconfigured`] if a required output or context bound is absent.
	pub fn estimate_tokens(&self, body: &[u8], output_max: Option<u64>, choices: u64) -> Result<u64, GatewayLimitError> {
		Ok(self.estimate_split(body, output_max, choices)?.total())
	}

	/// Estimates input and output tokens separately so each can be priced.
	///
	/// # Errors
	/// Returns [`GatewayLimitError::Misconfigured`] if a required output or context bound is absent.
	pub fn estimate_split(&self, body: &[u8], output_max: Option<u64>, choices: u64) -> Result<TokenEstimate, GatewayLimitError> {
		let output = output_max
			.or(self.max_output_tokens.map(u64::from))
			.filter(|value| *value > 0)
			.ok_or(GatewayLimitError::Misconfigured)?;
		let input = if has_external_input(body) {
			self.context_length.map(u64::from).ok_or(GatewayLimitError::Misconfigured)?
		} else {
			u64::try_from(body.len()).unwrap_or(u64::MAX).saturating_mul(2).saturating_add(1024)
		};
		let choices = choices.max(1);
		Ok(TokenEstimate {
			input: input.saturating_mul(choices),
			output: output.saturating_mul(choices),
		})
	}

	/// Bounds a Chat Completions request; the output maximum is applied only when token rules reserve it.
	pub fn chat_estimate(&self, policy: &GatewayRatePolicy, request: &mut OpenAIChatRequest) -> GatewayEstimate {
		let limited = policy.has_token_rules();
		let output_max = request.max_completion_tokens.or(request.max_tokens).or(self.max_output_tokens);
		if limited && request.max_completion_tokens.is_none() && request.max_tokens.is_none() {
			request.max_completion_tokens = output_max;
		}
		let result = serde_json::to_vec(&*request)
			.map_err(|_| GatewayLimitError::Unavailable)
			.and_then(|body| self.estimate_split(&body, output_max.map(u64::from), u64::from(request.n.unwrap_or(1))));
		GatewayEstimate { result, limited }
	}

	/// Bounds a Responses request; the output maximum is applied only when token rules reserve it.
	pub fn responses_estimate(&self, policy: &GatewayRatePolicy, request: &mut OpenAIResponsesRequestPayload) -> GatewayEstimate {
		let limited = policy.has_token_rules();
		if limited && request.max_output_tokens.is_none() {
			request.max_output_tokens = self.max_output_tokens.map(i64::from);
		}
		let output_max = request.max_output_tokens.and_then(|value| u64::try_from(value).ok());
		let result = serde_json::to_vec(&*request)
			.map_err(|_| GatewayLimitError::Unavailable)
			.and_then(|body| self.estimate_split(&body, output_max, 1));
		GatewayEstimate { result, limited }
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
