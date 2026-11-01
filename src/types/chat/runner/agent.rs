use super::history::tool_message_name;
use super::prepare::PreparedTurn;
use super::tools::{execute_tool_by_name, get_client_tool_info};
use crate::ai;
use crate::types::{
	ChatMessageResponse, ChatUsageGuard, PendingCall, CostDetails, JobState, Message, MessagePart, ReasoningDetails, RequestSettings, StreamData,
	StreamingAssistantMessageCreate, ToolExecution, ToolExecutionInternal, UsageDetails,
};
use axum::response::sse::Event;
use futures_util::{Stream, StreamExt};
use omniference::{
	stream::StreamEvent,
	types::{ChatRequestIR, ContentPart, Message as OmniMessage, Role},
};
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Instant};
use uuid::Uuid;

const MAX_ITERATIONS: usize = 10;

pub(crate) struct ToolCall {
	pub(crate) id: String,
	name: String,
	args: serde_json::Value,
}

pub(crate) struct ToolOutput {
	pub(crate) output: serde_json::Value,
	pub(crate) error: Option<String>,
	pub(crate) tool_id: Option<Uuid>,
	pub(crate) function_id: Option<Uuid>,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum Finish {
	Continue,
	Stop(StreamData),
}

#[derive(Default)]
struct IterationState {
	pending_tool_calls: HashMap<String, (String, String)>,
	tool_results: Vec<ToolExecutionInternal>,
	content: String,
	reasoning: String,
	completed_tool_calls: Vec<(String, String, serde_json::Value)>,
	provider_cost: Option<f64>,
}

pub(crate) struct AgentRun {
	state: Arc<JobState>,
	pub(crate) usage: ChatUsageGuard,
	user_id: Uuid,
	chat_id: Uuid,
	model_id: Uuid,
	assistant_parent_id: Option<Uuid>,
	assistant_fork_index: i32,
	reasoning_effort: Option<String>,
	reasoning_budget_tokens: Option<u32>,
	request_settings: RequestSettings,
	start_time: Instant,
	base_ir: ChatRequestIR,
	current_messages: Vec<OmniMessage>,
	iteration: usize,
	full_content: String,
	reasoning_content: String,
	reasoning_start: Option<Instant>,
	reasoning_latency_ms: Option<i32>,
	cost_details: CostDetails,
	all_tool_executions: Vec<ToolExecutionInternal>,
	assistant_message_parts: Vec<MessagePart>,
	step: IterationState,
}

fn sse(data: StreamData) -> Result<Event, Infallible> {
	Ok(Event::default().data(serde_json::to_string(&data).unwrap_or_default()))
}

fn error_event(code: &str, message: impl Into<String>) -> StreamData {
	StreamData::Error {
		code: code.to_string(),
		message: message.into(),
	}
}

impl AgentRun {
	pub(crate) fn new(state: Arc<JobState>, turn: PreparedTurn, start_time: Instant) -> Self {
		let mut base_ir = turn.ir;
		let current_messages = std::mem::take(&mut base_ir.messages);
		Self {
			state,
			usage: turn.usage,
			user_id: turn.user_id,
			chat_id: turn.chat_id,
			model_id: turn.model_id,
			assistant_parent_id: turn.assistant_parent_id,
			assistant_fork_index: turn.assistant_fork_index,
			reasoning_effort: turn.reasoning_effort,
			reasoning_budget_tokens: turn.reasoning_budget_tokens,
			request_settings: turn.request_settings,
			start_time,
			base_ir,
			current_messages,
			iteration: 0,
			full_content: String::new(),
			reasoning_content: String::new(),
			reasoning_start: None,
			reasoning_latency_ms: None,
			cost_details: CostDetails::default(),
			all_tool_executions: Vec::new(),
			assistant_message_parts: Vec::new(),
			step: IterationState::default(),
		}
	}

	#[allow(clippy::result_large_err)]
	pub(crate) fn next_request(&mut self) -> Result<ChatRequestIR, StreamData> {
		self.iteration += 1;
		if self.iteration > MAX_ITERATIONS {
			tracing::warn!(max_iterations = MAX_ITERATIONS, "max agentic iterations reached");
			return Err(error_event("max_iterations", "Maximum tool call iterations reached"));
		}
		self.step = IterationState::default();
		self.usage.start_iteration();
		let mut ir = self.base_ir.clone();
		ir.messages = self.current_messages.clone();
		Ok(ir)
	}

	fn chat_failed(&self, error: &impl std::fmt::Display) -> StreamData {
		tracing::error!(%error, iteration = self.iteration, "failed to start chat iteration");
		error_event("chat_error", error.to_string())
	}

	pub(crate) fn apply(&mut self, event: StreamEvent) -> Option<StreamData> {
		match event {
			StreamEvent::TextDelta { content } => {
				if let Some(start) = self.reasoning_start.take() {
					self.reasoning_latency_ms = Some(start.elapsed().as_millis() as i32);
				}
				self.full_content.push_str(&content);
				self.step.content.push_str(&content);
				Some(StreamData::TextDelta { content })
			}
			StreamEvent::ReasoningDelta { content } => {
				if self.reasoning_start.is_none() {
					self.reasoning_start = Some(Instant::now());
				}
				self.reasoning_content.push_str(&content);
				self.step.reasoning.push_str(&content);
				Some(StreamData::ReasoningDelta { content })
			}
			StreamEvent::ToolCallStart { id, name, args_json } => {
				self.step.pending_tool_calls.insert(id.clone(), (name.clone(), args_json.to_string()));
				Some(StreamData::ToolCallStart { id, name })
			}
			StreamEvent::ToolCallDelta { id, args_delta_json } => {
				let delta_str = match &args_delta_json {
					serde_json::Value::String(s) => s.clone(),
					other => other.to_string(),
				};
				if let Some((_, args)) = self.step.pending_tool_calls.get_mut(&id) {
					args.push_str(&delta_str);
				}
				Some(StreamData::ToolCallDelta { id, args_delta: delta_str })
			}
			StreamEvent::Tokens { input, output } => {
				self.usage.input_tokens += input;
				self.usage.output_tokens += output;
				Some(StreamData::Tokens {
					input: self.usage.input_tokens,
					output: self.usage.output_tokens,
					reasoning: if self.usage.reasoning_tokens > 0 { Some(self.usage.reasoning_tokens) } else { None },
				})
			}
			StreamEvent::OpenAIMetadata { completion_tokens_details, .. } => {
				if let Some(details) = completion_tokens_details
					&& details.reasoning_tokens > 0 {
						self.usage.reasoning_tokens += details.reasoning_tokens;
					}
				None
			}
			StreamEvent::Error { code, message } => Some(StreamData::Error { code, message }),
			StreamEvent::Cost { cost } => {
				self.cost_details.input = cost.prompt;
				self.cost_details.output = cost.completion;
				self.cost_details.reasoning = cost.reasoning;
				self.cost_details.total = Some(cost.total);
				self.step.provider_cost = Some(cost.total);
				Some(StreamData::Usage { cost_details: self.cost_details })
			}
			_ => None,
		}
	}

	pub(crate) fn take_pending_call(&mut self, id: String, args: serde_json::Value) -> Option<ToolCall> {
		let (name, _) = self.step.pending_tool_calls.remove(&id)?;
		self.step.completed_tool_calls.push((id.clone(), name.clone(), args.clone()));
		Some(ToolCall { id, name, args })
	}

	async fn client_tool(&self, call: &ToolCall) -> Option<(StreamData, Uuid, PendingCall)> {
		let (mcp_server_id, mcp_tool_name, tool_id) = get_client_tool_info(&self.state.db, &call.name, self.user_id).await?;
		let event = StreamData::ClientToolCall {
			id: call.id.clone(),
			name: call.name.clone(),
			args: call.args.clone(),
			mcp_server_id,
			mcp_tool_name,
		};
		let pending = self.state.client_tool_pending.register(call.id.clone(), self.user_id).await;
		Some((event, tool_id, pending))
	}

	async fn await_client_result(&self, call_id: &str, tool_id: Uuid, mut pending: PendingCall) -> ToolOutput {
		let output = pending.wait(std::time::Duration::from_secs(120)).await.unwrap_or_else(|| {
			tracing::warn!(call_id, "client tool call timed out or failed");
			serde_json::json!({"error": "Client tool execution timed out"})
		});
		ToolOutput {
			output,
			error: None,
			tool_id: Some(tool_id),
			function_id: None,
		}
	}

	async fn run_server_tool(&self, call: &ToolCall) -> ToolOutput {
		match execute_tool_by_name(&self.state.db, &self.state.mcp_pool, self.user_id, &call.name, call.args.clone()).await {
			Ok(result) => ToolOutput {
				output: result.output,
				error: None,
				tool_id: Some(result.tool_id),
				function_id: result.function_id,
			},
			Err(e) => ToolOutput {
				output: serde_json::json!({"error": e}),
				error: Some(e),
				tool_id: None,
				function_id: None,
			},
		}
	}

	pub(crate) fn record_tool_result(&mut self, call: ToolCall, started: Instant, result: ToolOutput) -> StreamData {
		let ToolOutput { output, error, tool_id, function_id } = result;
		self.step.tool_results.push(ToolExecutionInternal {
			call_id: call.id.clone(),
			tool_name: call.name.clone(),
			args: call.args,
			output: output.clone(),
			error: error.clone(),
			execution_ms: started.elapsed().as_millis() as i32,
			tool_id,
			function_id,
		});
		StreamData::ToolResult {
			id: call.id,
			output,
			error,
			tool_id,
			tool_function: function_id,
			tool_name: Some(call.name),
		}
	}

	pub(crate) async fn finish_iteration(&mut self) -> Finish {
		if let Some(cost) = self.step.provider_cost.take() {
			self.usage.add_provider_cost(cost);
		}
		if self.step.tool_results.is_empty() {
			Finish::Stop(self.finalize_turn().await)
		} else {
			self.continue_with_tools();
			Finish::Continue
		}
	}

	fn continue_with_tools(&mut self) {
		let mut assistant_parts: Vec<ContentPart> = Vec::new();
		if !self.step.reasoning.is_empty() {
			self.assistant_message_parts.push(MessagePart::Reasoning { text: self.step.reasoning.clone() });
		}
		if !self.step.content.is_empty() {
			assistant_parts.push(ContentPart::Text(self.step.content.clone()));
			self.assistant_message_parts.push(MessagePart::Text { text: self.step.content.clone() });
		}
		for (call_id, tool_name, args) in self.step.completed_tool_calls.drain(..) {
			let arguments = match &args {
				serde_json::Value::String(s) => s.clone(),
				other => other.to_string(),
			};
			self.assistant_message_parts.push(MessagePart::ToolCall {
				id: call_id.clone(),
				name: tool_name.clone(),
				arguments: arguments.clone(),
			});
			assistant_parts.push(ContentPart::ToolCall {
				id: call_id,
				name: tool_name,
				arguments,
			});
		}
		if !assistant_parts.is_empty() {
			self.current_messages.push(OmniMessage {
				role: Role::Assistant,
				parts: assistant_parts,
				name: None,
			});
		}

		for exec in self.step.tool_results.drain(..) {
			if matches!(exec.tool_name.as_str(), "imagegen" | "imagegen_generate" | "imagegen_edit")
				&& exec.error.is_none()
				&& let Some(image_id) = exec.output.get("image_id").and_then(serde_json::Value::as_str)
				&& Uuid::parse_str(image_id).is_ok()
			{
				self.assistant_message_parts.push(MessagePart::Image { image_id: image_id.to_string() });
			}
			let result_text = if let Some(ref err) = exec.error {
				format!("Error: {}", err)
			} else {
				serde_json::to_string(&exec.output).unwrap_or_else(|_| exec.output.to_string())
			};

			self.current_messages.push(OmniMessage {
				role: Role::Tool,
				parts: vec![ContentPart::Text(result_text)],
				name: Some(tool_message_name(&exec.tool_name, &exec.call_id)),
			});
			self.assistant_message_parts.push(MessagePart::ToolResult { tool_call_id: exec.call_id.clone() });

			self.all_tool_executions.push(exec);
		}
	}

	async fn save_message(&mut self) -> Result<Message, sqlx::Error> {
		let usage_details = UsageDetails {
			input_tokens: Some(self.usage.input_tokens as i32),
			output_tokens: Some(self.usage.output_tokens as i32),
			reasoning_tokens: Some(self.usage.reasoning_tokens as i32),
			latency_ms: Some(self.start_time.elapsed().as_millis() as i32),
			reasoning_latency_ms: self.reasoning_latency_ms,
		};
		let reasoning_details = ReasoningDetails {
			effort: self.reasoning_effort.clone(),
			budget_tokens: self.reasoning_budget_tokens.map(|b| b as i32),
		};

		if !self.step.reasoning.is_empty() {
			self.assistant_message_parts.push(MessagePart::Reasoning { text: self.step.reasoning.clone() });
		}
		if !self.step.content.is_empty() {
			self.assistant_message_parts.push(MessagePart::Text { text: self.step.content.clone() });
		}
		let has_tool_calls = self.assistant_message_parts.iter().any(|p| matches!(p, MessagePart::ToolCall { .. }));
		let content_parts_json = if has_tool_calls {
			serde_json::to_value(&self.assistant_message_parts).ok()
		} else {
			None
		};

		Message::create_streaming_assistant(
			&self.state.db,
			StreamingAssistantMessageCreate {
				chat_id: &self.chat_id,
				content: &self.full_content,
				content_parts: content_parts_json.as_ref(),
				reasoning_content: if self.reasoning_content.is_empty() { None } else { Some(&self.reasoning_content) },
				model_id: &self.model_id,
				reasoning_details: &reasoning_details,
				usage_details: &usage_details,
				cost_details: &self.cost_details,
				request_settings: &self.request_settings,
				parent_id: self.assistant_parent_id,
				fork_index: self.assistant_fork_index,
			},
		)
		.await
	}

	#[allow(clippy::cognitive_complexity)]
	async fn finalize_turn(&mut self) -> StreamData {
		let msg = match self.save_message().await {
			Ok(msg) => msg,
			Err(error) => {
				tracing::error!(%error, "failed to save assistant message");
				return error_event("save_failed", "Failed to save response");
			}
		};
		if let Err(error) = self.usage.record().await {
			tracing::error!(%error, message_id = %msg.id, "failed to record usage event");
			return error_event("accounting_failed", "Failed to record usage");
		}
		if !self.all_tool_executions.is_empty()
			&& let Err(error) = ToolExecution::create_for_message_batch(&self.state.db, &msg.id, &self.all_tool_executions).await {
				tracing::error!(%error, message_id = %msg.id, "failed to save tool executions");
			}
		let message: ChatMessageResponse = msg.into();
		StreamData::Done { message }
	}
}

/// Streams one chat turn to the client as SSE events.
///
/// The usage guard lives inside the returned stream, so dropping the stream (client disconnect)
/// cancels the loop and records any consumed tokens.
pub fn stream_turn(state: Arc<JobState>, turn: PreparedTurn, start_time: Instant) -> impl Stream<Item = Result<Event, Infallible>> {
	async_stream::stream! {
		if let Some(msg) = &turn.user_message {
			let message: ChatMessageResponse = msg.clone().into();
			yield sse(StreamData::UserMessageSaved { message });
		}

		let engine = ai::get();
		let mut run = AgentRun::new(state, turn, start_time);

		'agentic_loop: loop {
			let request = match run.next_request() {
				Ok(request) => request,
				Err(event) => {
					yield sse(event);
					break 'agentic_loop;
				}
			};
			let mut upstream = match engine.chat(request).await {
				Ok(upstream) => upstream,
				Err(error) => {
					yield sse(run.chat_failed(&error));
					break 'agentic_loop;
				}
			};

			while let Some(event) = upstream.next().await {
				match event {
					StreamEvent::ToolCallEnd { id, args_json } => {
						yield sse(StreamData::ToolCallEnd { id: id.clone() });
						if let Some(call) = run.take_pending_call(id, args_json) {
							let started = Instant::now();
							let result = match run.client_tool(&call).await {
								Some((event, tool_id, pending)) => {
									yield sse(event);
									run.await_client_result(&call.id, tool_id, pending).await
								}
								None => run.run_server_tool(&call).await,
							};
							yield sse(run.record_tool_result(call, started, result));
						}
					}
					StreamEvent::Done => match run.finish_iteration().await {
						Finish::Continue => continue 'agentic_loop,
						Finish::Stop(event) => {
							yield sse(event);
							break 'agentic_loop;
						}
					},
					other => {
						if let Some(data) = run.apply(other) {
							yield sse(data);
						}
					}
				}
			}
			break 'agentic_loop;
		}
	}
}
