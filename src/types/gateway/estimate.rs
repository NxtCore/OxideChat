use super::{GatewayInference, GatewayLimitError, GatewayRatePolicy};
use crate::types::{TokenEstimate, UnitEstimate};
use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
use serde_json::{Map, Value};

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

/// Audio bytes per second assumed for uploads, a low speech bitrate so the duration is not underestimated.
const AUDIO_BYTES_PER_SECOND: u64 = 2_000;
/// Audio input tokens per second of speech for models priced in audio tokens.
const AUDIO_TOKENS_PER_SECOND: u64 = 25;
/// Transcript tokens per second of speech.
const TRANSCRIPT_TOKENS_PER_SECOND: u64 = 8;
/// Audio output tokens per input character for speech models priced in audio tokens.
const SPEECH_TOKENS_PER_CHARACTER: u64 = 2;
/// Input tokens reserved for every image uploaded to an image edit.
const IMAGE_INPUT_TOKENS: u64 = 6_000;
/// Output tokens reserved for every generated image.
const IMAGE_OUTPUT_TOKENS: u64 = 8_000;

#[derive(Clone, Copy, Debug)]
/// What a non-chat request is, as far as estimating it needs; its text fields are passed alongside.
pub enum MediaKind {
	/// `/embeddings`.
	Embeddings,
	/// `/rerank`, billed per search where the model has that rate.
	Rerank,
	/// `/decisions`, whose input may carry images.
	Decision,
	/// `/images/generations` and `/images/edits`, with the number of uploaded images.
	Image { images: u64 },
	/// `/audio/transcriptions` and `/audio/translations`, with the size of the uploaded audio.
	Transcription { audio_bytes: u64 },
	/// `/audio/speech`.
	Speech,
}

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
		let input = self.estimate_input(body)?;
		let choices = choices.max(1);
		Ok(TokenEstimate::text(input.saturating_mul(choices), output.saturating_mul(choices)))
	}

	fn estimate_input(&self, body: &[u8]) -> Result<u64, GatewayLimitError> {
		if has_external_input(body) {
			return self.context_length.map(u64::from).ok_or(GatewayLimitError::Misconfigured);
		}
		Ok(TokenEstimate::input_for_bytes(u64::try_from(body.len()).unwrap_or(u64::MAX)))
	}

	/// Bounds a non-chat request from its JSON body or form text fields: text input plus the units
	/// each kind is billed by (one request, searches, audio seconds and tokens, characters, images).
	pub fn media_estimate(&self, policy: &GatewayRatePolicy, kind: MediaKind, fields: &Map<String, Value>) -> GatewayEstimate {
		let result = serde_json::to_vec(fields).map_err(|_| GatewayLimitError::Unavailable).and_then(|body| match kind {
			MediaKind::Decision => self.estimate_input(&body),
			_ => Ok(TokenEstimate::input_for_bytes(body.len() as u64)),
		});
		let result = result.map(|input| media_tokens(kind, fields, input));
		GatewayEstimate {
			result,
			limited: policy.has_token_rules(),
		}
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

	/// Bounds an Anthropic Messages request; the output maximum is applied only when token rules reserve it.
	pub fn messages_estimate(&self, policy: &GatewayRatePolicy, request: &mut Map<String, Value>) -> GatewayEstimate {
		let limited = policy.has_token_rules();
		if limited && !request.contains_key("max_tokens") && let Some(max) = self.max_output_tokens {
			request.insert("max_tokens".to_owned(), Value::from(max));
		}
		let output_max = request.get("max_tokens").and_then(Value::as_u64);
		let result = serde_json::to_vec(&*request)
			.map_err(|_| GatewayLimitError::Unavailable)
			.and_then(|body| self.estimate_split(&body, output_max, 1));
		GatewayEstimate { result, limited }
	}
}

/// Bounds one image the web chat image tool generates from a prompt and its input images.
#[must_use]
pub fn image_tool_estimate(prompt_bytes: u64, input_images: u64) -> TokenEstimate {
	media_tokens(MediaKind::Image { images: input_images }, &Map::new(), TokenEstimate::input_for_bytes(prompt_bytes))
}

/// Bounds one knowledge embedding or rerank request over `bytes` of text, counting the request and, for a rerank, the search.
#[must_use]
pub fn knowledge_estimate(bytes: u64, kind: MediaKind) -> TokenEstimate {
	media_tokens(kind, &Map::new(), TokenEstimate::input_for_bytes(bytes))
}

fn media_tokens(kind: MediaKind, fields: &Map<String, Value>, input: u64) -> TokenEstimate {
	let mut units = UnitEstimate { requests: 1, ..UnitEstimate::default() };
	let mut estimate = TokenEstimate::text(input, 0);
	match kind {
		MediaKind::Embeddings | MediaKind::Decision => {}
		MediaKind::Rerank => units.searches = 1,
		MediaKind::Image { images } => {
			let count = image_count(fields);
			units.requests = u32::try_from(count).unwrap_or(u32::MAX);
			estimate.input = input.saturating_add(images.saturating_mul(IMAGE_INPUT_TOKENS));
			estimate.output = count.saturating_mul(IMAGE_OUTPUT_TOKENS);
		}
		MediaKind::Transcription { audio_bytes } => {
			let seconds = audio_bytes.div_ceil(AUDIO_BYTES_PER_SECOND).max(1);
			units.audio_seconds = seconds;
			units.input_audio_tokens = seconds.saturating_mul(AUDIO_TOKENS_PER_SECOND);
			estimate.output = seconds.saturating_mul(TRANSCRIPT_TOKENS_PER_SECOND);
		}
		MediaKind::Speech => {
			let characters = fields.get("input").and_then(Value::as_str).map_or(0, |input| input.chars().count() as u64);
			units.characters = characters;
			units.output_audio_tokens = characters.saturating_mul(SPEECH_TOKENS_PER_CHARACTER);
		}
	}
	estimate.units = units;
	estimate
}

fn image_count(fields: &Map<String, Value>) -> u64 {
	let count = match fields.get("n") {
		Some(Value::Number(number)) => number.as_u64(),
		Some(Value::String(text)) => text.trim().parse().ok(),
		_ => None,
	};
	count.unwrap_or(1).max(1)
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
