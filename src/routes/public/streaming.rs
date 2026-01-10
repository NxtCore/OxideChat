//! Streaming routes for AI responses.
//!
//! Server-Sent Events (SSE) endpoint for streaming AI chat completions.

use crate::AppState;
use crate::ai;
use crate::routes::public::auth::get_current_user;
use crate::types::ai::ModelConfig;
use crate::types::{ChatMessageResponse, Message, Tool, ToolFunction, UserToolSettings};
use crate::utils::tools::{HttpExecutor, ToolContext, ToolExecutor, get_builtin_executor};
use axum::{
	Json,
	extract::{Path, State},
	response::{
		IntoResponse,
		sse::{Event, KeepAlive, Sse},
	},
};
use futures_util::StreamExt;
use omniference::Sampling;
use omniference::{
	stream::StreamEvent,
	types::{ChatRequestIR, ContentPart, Message as OmniMessage, Role, ToolChoice, ToolSpec},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Instant};
use tower_cookies::Cookies;
use uuid::Uuid;

/// Request body for sending a message and streaming AI response
#[derive(Debug, Deserialize)]
pub struct StreamRequest {
	pub content: String,
	pub model_key: String,
	pub reasoning_effort: Option<String>,
	pub reasoning_budget_tokens: Option<u32>,
	pub tools_enabled: Option<Vec<String>>,
	pub sampling: Option<Sampling>,
}

/// SSE event data
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamData {
	/// User message saved confirmation
	UserMessageSaved { message: ChatMessageResponse },
	/// Text content delta
	TextDelta { content: String },
	/// Reasoning text delta (for models that support it)
	ReasoningDelta { content: String },
	/// Tool call started
	ToolCallStart { id: String, name: String },
	/// Tool call argument delta
	ToolCallDelta { id: String, args_delta: String },
	/// Tool call ended (arguments complete)
	ToolCallEnd { id: String },
	/// Tool execution result
	ToolResult {
		id: String,
		output: serde_json::Value,
		error: Option<String>,
		tool_id: Option<Uuid>,
		tool_function: Option<Uuid>,
		tool_name: Option<String>,
	},
	/// Token count update
	Tokens { input: u32, output: u32, reasoning: Option<u32> },
	/// Error occurred
	Error { code: String, message: String },
	/// Stream completed with message info
	Done { message: ChatMessageResponse },
}

fn error_stream(code: impl Into<String>, message: impl Into<String>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
	let code = code.into();
	let message = message.into();
	let data = serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default();
	Sse::new(futures_util::stream::once(async move { Ok::<_, Infallible>(Event::default().data(data)) })).keep_alive(KeepAlive::default())
}

/// Merge sampling options with priority order:
/// 1. Request-level (highest priority)
/// 2. model_configs (user preferences)
/// 3. models table (defaults)
fn merge_sampling_with_priority(request_sampling: Option<&Sampling>, model_config: Option<&ModelConfig>, model: &crate::types::AiModel) -> Sampling {
	let config_sampling: Option<Sampling> = model_config.and_then(|mc| serde_json::from_value(mc.sampling.clone()).ok());

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
			.or_else(|| model.max_tokens.map(|t| t as u32)),
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
/// 2. model_configs extra_settings
fn merge_reasoning_with_priority(
	request_effort: Option<&String>,
	request_budget: Option<u32>,
	model_config: Option<&ModelConfig>,
) -> Option<omniference::types::ReasoningConfig> {
	let config_effort: Option<String> = model_config
		.and_then(|mc| mc.extra_settings.get("reasoning_effort"))
		.and_then(|v| v.as_str())
		.map(|s| s.to_string());

	let config_budget: Option<u32> = model_config
		.and_then(|mc| mc.extra_settings.get("reasoning_budget_tokens"))
		.and_then(|v| v.as_u64())
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

async fn execute_tool_by_name(db: &sqlx::PgPool, user_id: Uuid, full_tool_name: &str, input: serde_json::Value) -> Result<crate::types::ToolExecutionResult, String> {
	use crate::types::ToolSourceKind;

	let mut tool: Option<Tool> = None;
	let mut function_name: Option<String> = None;
	let mut function_id: Option<Uuid> = None;

	tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE name = $1 AND is_enabled = true AND (is_public = true OR owner_id = $2)")
		.bind(full_tool_name)
		.bind(user_id)
		.fetch_optional(db)
		.await
		.map_err(|e| format!("Database error: {e}"))?;

	if tool.is_none() && full_tool_name.contains('_') {
		let underscores: Vec<_> = full_tool_name.match_indices('_').collect();
		for (pos, _) in underscores.iter().rev() {
			let potential_tool_name = &full_tool_name[..*pos];
			let potential_func_name = &full_tool_name[pos + 1..];

			let found = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE name = $1 AND is_enabled = true AND (is_public = true OR owner_id = $2)")
				.bind(potential_tool_name)
				.bind(user_id)
				.fetch_optional(db)
				.await
				.map_err(|e| format!("Database error: {e}"))?;

			if found.is_some() {
				tool = found;
				function_name = Some(potential_func_name.to_string());
				break;
			}
		}
	}

	let tool = tool.ok_or_else(|| format!("Tool '{}' not found", full_tool_name))?;

	let function_entrypoint = if let Some(fn_name) = &function_name {
		let func = sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 AND name = $2")
			.bind(tool.id)
			.bind(fn_name)
			.fetch_optional(db)
			.await
			.map_err(|e| format!("Database error: {e}"))?
			.ok_or_else(|| format!("Function '{}' not found in tool '{}'", fn_name, tool.name))?;
		function_id = Some(func.id);
		func.entrypoint
	} else {
		None
	};

	let settings = sqlx::query_as::<_, UserToolSettings>(
		"SELECT * FROM user_tool_settings 
		WHERE tool_id = $1 
		ORDER BY (CASE WHEN user_id = ($2::uuid) THEN 0 ELSE 1 END) 
		LIMIT 1",
	)
	.bind(tool.id)
	.bind(user_id)
	.fetch_optional(db)
	.await
	.ok()
	.flatten()
	.map(|s| s.settings)
	.unwrap_or_else(|| serde_json::json!({}));

	let ctx = ToolContext {
		user_id: Some(user_id),
		settings,
		timeout_ms: Some(30000),
		function_name: function_name.map(|s| s.to_string()),
	};

	match tool.source_kind {
		ToolSourceKind::Builtin => {
			let builtin_id = tool
				.source_config
				.get("builtin_id")
				.and_then(|v| v.as_str())
				.ok_or("Missing builtin_id in source_config")?;

			let executor = get_builtin_executor(builtin_id).map_err(|e| format!("{:?}", e))?;
			let output = executor.execute(input, &ctx).await.map_err(|e| format!("{:?}", e))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		ToolSourceKind::Http => {
			let base_url = tool.source_config.get("url").and_then(|v| v.as_str()).unwrap_or("");
			let url = if let Some(entrypoint) = &function_entrypoint {
				if entrypoint.starts_with("http") {
					entrypoint.clone()
				} else {
					format!("{}{}", base_url.trim_end_matches('/'), entrypoint)
				}
			} else {
				base_url.to_string()
			};

			let headers: HashMap<String, String> = tool
				.source_config
				.get("headers")
				.and_then(|v| serde_json::from_value(v.clone()).ok())
				.unwrap_or_default();

			let http_config = crate::utils::tools::http::HttpConfig {
				method: tool.source_config.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_string(),
				url,
				headers,
				body_template: tool.source_config.get("body_template").and_then(|v| v.as_str()).map(String::from),
			};

			let executor = HttpExecutor::new(full_tool_name.to_string(), http_config).map_err(|e| format!("{:?}", e))?;
			let output = executor.execute(input, &ctx).await.map_err(|e| format!("{:?}", e))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		_ => Err(format!("Unsupported tool source: {:?}", tool.source_kind)),
	}
}

pub async fn stream_completion(State(state): State<Arc<AppState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<StreamRequest>) -> impl IntoResponse {
	eprint!("[STREAM] Starting stream completion");

	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return error_stream("not_authenticated", "Authentication required").into_response(),
	};

	let chat = sqlx::query_as::<_, crate::types::Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
		.bind(chat_id)
		.bind(user.id)
		.fetch_optional(&state.db)
		.await;

	let _chat = match chat {
		Ok(Some(chat)) => chat,
		Ok(None) => return error_stream("not_found", "Chat not found").into_response(),
		Err(e) => {
			eprintln!("[STREAM] Failed to fetch chat: {e}");
			return error_stream("internal_error", "Failed to fetch chat").into_response();
		}
	};

	eprintln!("[STREAM] Chat verified");

	let model = sqlx::query_as::<_, crate::types::AiModel>("SELECT * FROM models WHERE model_id = $1")
		.bind(&req.model_key)
		.fetch_optional(&state.db)
		.await;

	let model = match model {
		Ok(Some(m)) => m,
		Ok(None) => return error_stream("not_found", "Model not found").into_response(),
		Err(e) => {
			eprintln!("[STREAM] Failed to fetch model: {e}");
			return error_stream("internal_error", "Failed to fetch model").into_response();
		}
	};

	eprintln!("[STREAM] Model verified");

	let model_config = sqlx::query_as::<_, ModelConfig>("SELECT * FROM model_configs WHERE owner_id = $1 AND stable_key = $2")
		.bind(user.id)
		.bind(&req.model_key)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	eprintln!("[STREAM] Model config: {:?}", model_config.as_ref().map(|mc| &mc.name));

	let reasoning_details = crate::types::ReasoningDetails {
		effort: req.reasoning_effort.clone(),
		budget_tokens: req.reasoning_budget_tokens.map(|b| b as i32),
	};
	let usage_details = crate::types::UsageDetails::default();
	let cost_details = crate::types::CostDetails::default();

	let user_message = sqlx::query_as::<_, Message>(
		r#"
		INSERT INTO messages (chat_id, role, content, model_id, reasoning_details, usage_details, cost_details)
		VALUES ($1, 'user', $2, $3, $4, $5, $6)
		RETURNING *
		"#,
	)
	.bind(chat_id)
	.bind(&req.content)
	.bind(model.id)
	.bind(sqlx::types::Json(reasoning_details))
	.bind(sqlx::types::Json(usage_details))
	.bind(sqlx::types::Json(cost_details))
	.fetch_one(&state.db)
	.await;

	let user_message = match user_message {
		Ok(msg) => msg,
		Err(e) => {
			eprintln!("[STREAM] Failed to save user message: {e}");
			return error_stream("save_failed", "Failed to save message").into_response();
		}
	};

	eprintln!("[STREAM] User message saved");

	let messages = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 ORDER BY created_at ASC")
		.bind(chat_id)
		.fetch_all(&state.db)
		.await;

	let messages = match messages {
		Ok(msgs) => msgs,
		Err(e) => {
			eprintln!("[STREAM] Failed to fetch messages: {e}");
			return error_stream("internal_error", "Failed to fetch messages").into_response();
		}
	};

	eprintln!("[STREAM] Messages fetched");

	let engine = ai::get();
	let engine_read = engine.read().await;

	let omni_model = match engine_read.get_model(&req.model_key).await {
		Some(m) => m,
		None => {
			let msg = format!("Model '{}' not found", req.model_key);
			drop(engine_read);
			return error_stream("model_not_found", msg).into_response();
		}
	};

	eprintln!("[STREAM] Model verified");

	let provider = match engine_read.get_provider(&omni_model.provider_name).await {
		Some(p) => p,
		None => {
			let msg = format!("Provider '{}' not found", omni_model.provider_name);
			drop(engine_read);
			return error_stream("provider_not_found", msg).into_response();
		}
	};

	eprintln!("[STREAM] Provider verified");

	let omni_messages: Vec<OmniMessage> = messages
		.iter()
		.filter(|m| m.role == "user" || m.role == "assistant")
		.map(|m| OmniMessage {
			role: if m.role == "user" { Role::User } else { Role::Assistant },
			parts: vec![ContentPart::Text(m.content.clone())],
			name: None,
		})
		.collect();

	eprintln!("[STREAM] Messages built");
	let mut ir = ChatRequestIR::default();
	ir.model.alias = omni_model.id.clone();
	ir.model.model_id = omni_model.id.clone();
	ir.model.provider = provider;
	ir.model.input_modalities = omni_model.input_modalities.clone();
	ir.model.output_modalities = omni_model.output_modalities.clone();
	ir.messages = omni_messages;
	ir.stream = true;
	ir.metadata.insert("user_id".to_string(), user.id.to_string());
	ir.metadata.insert("chat_id".to_string(), chat_id.to_string());
	ir.sampling = merge_sampling_with_priority(req.sampling.as_ref(), model_config.as_ref(), &model);
	ir.reasoning = merge_reasoning_with_priority(req.reasoning_effort.as_ref(), req.reasoning_budget_tokens, model_config.as_ref());

	if let Some(ref enabled_tool_ids) = req.tools_enabled {
		eprintln!("[STREAM] Tools enabled in request: {:?}", enabled_tool_ids);
		if !enabled_tool_ids.is_empty() {
			let tool_uuids: Vec<Uuid> = enabled_tool_ids.iter().filter_map(|id| Uuid::parse_str(id).ok()).collect();
			eprintln!("[STREAM] Parsed {} UUIDs: {:?}", tool_uuids.len(), tool_uuids);

			if tool_uuids.is_empty() {
				eprintln!("[STREAM] No valid tool UUIDs found in request");
			}

			let tools = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = ANY($1) AND is_enabled = true AND (is_public = true OR owner_id = $2)")
				.bind(&tool_uuids)
				.bind(user.id)
				.fetch_all(&state.db)
				.await;

			eprintln!("[STREAM] Tools query result: {:?}", tools.as_ref().map(|t| t.len()).map_err(|e| e.to_string()));

			if let Ok(tools) = tools {
				eprintln!("[STREAM] Found {} tools from DB", tools.len());
				let tool_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();

				let functions: Vec<ToolFunction> = if tool_ids.is_empty() {
					vec![]
				} else {
					sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = ANY($1) ORDER BY sort_order, created_at")
						.bind(&tool_ids)
						.fetch_all(&state.db)
						.await
						.unwrap_or_default()
				};

				let mut functions_by_tool: HashMap<Uuid, Vec<ToolFunction>> = HashMap::new();
				for f in functions {
					functions_by_tool.entry(f.tool_id).or_default().push(f);
				}

				let mut tool_specs: Vec<ToolSpec> = vec![];
				for tool in tools {
					let funcs = functions_by_tool.get(&tool.id).map(|v| v.as_slice()).unwrap_or(&[]);
					tool_specs.extend(tool.to_tool_specs(funcs));
				}

				if !tool_specs.is_empty() {
					eprintln!("[STREAM] Loaded {} tool specs", tool_specs.len());
					ir.tools = tool_specs;
					ir.tool_choice = ToolChoice::Auto;
				}
			}
		}
	}

	eprintln!("[STREAM] Chat request: {:?}", ir);

	let omni_messages_for_stream = ir.messages.clone();
	let ir_for_stream = ir.clone();

	let db = state.db.clone();
	let start_time = Instant::now();
	let reasoning_effort = req.reasoning_effort.clone();
	let reasoning_budget_tokens = req.reasoning_budget_tokens;

	let sse_stream = async_stream::stream! {
	eprintln!("[STREAM] Stream generator started");
	let user_message_response: ChatMessageResponse = user_message.into();
	yield Ok::<_, Infallible>(Event::default().data(
		serde_json::to_string(&StreamData::UserMessageSaved { message: user_message_response }).unwrap_or_default()
	));

	eprintln!("[STREAM] About to start consuming upstream");

	let mut full_content = String::new();
	let mut reasoning_content = String::new();
	let mut input_tokens: u32 = 0;
	let mut output_tokens: u32 = 0;
	let mut reasoning_tokens: u32 = 0;
	let mut reasoning_start: Option<Instant> = None;
	let mut reasoning_latency_ms: Option<i32> = None;

	let mut current_messages = omni_messages_for_stream;
	let mut iteration = 0;
	const MAX_ITERATIONS: usize = 10;

	let mut all_tool_executions: Vec<(String, String, serde_json::Value, serde_json::Value, Option<String>, i32, Option<Uuid>, Option<Uuid>)> = Vec::new();

	let engine = ai::get();
	let tool_specs = ir_for_stream.tools.clone();
	let tool_choice = ir_for_stream.tool_choice.clone();
	let mut base_ir = ir_for_stream;

	'agentic_loop: loop {
				iteration += 1;
				if iteration > MAX_ITERATIONS {
					eprintln!("[STREAM] Max agentic iterations ({}) reached", MAX_ITERATIONS);
					yield Ok::<_, Infallible>(Event::default().data(
						serde_json::to_string(&StreamData::Error {
							code: "max_iterations".to_string(),
							message: "Maximum tool call iterations reached".to_string(),
						}).unwrap_or_default()
					));
					break 'agentic_loop;
				}

				eprintln!("[STREAM] Agentic loop iteration {}", iteration);

				base_ir.messages = current_messages.clone();
				base_ir.tools = tool_specs.clone();
				base_ir.tool_choice = tool_choice.clone();

				let engine_read = engine.read().await;
				let stream_result = engine_read.chat(base_ir.clone()).await;
				drop(engine_read);

				let upstream = match stream_result {
					Ok(s) => s,
					Err(e) => {
						eprintln!("[STREAM] Failed to start chat iteration {}: {}", iteration, e);
						yield Ok::<_, Infallible>(Event::default().data(
							serde_json::to_string(&StreamData::Error {
								code: "chat_error".to_string(),
								message: e.to_string(),
							}).unwrap_or_default()
						));
						break 'agentic_loop;
					}
				};

				tokio::pin!(upstream);

				let mut pending_tool_calls: HashMap<String, (String, String)> = HashMap::new();
				let mut tool_results: Vec<(String, String, serde_json::Value, serde_json::Value, Option<String>, i32, Option<Uuid>, Option<Uuid>)> = Vec::new();
				let mut iteration_content = String::new();

				let mut event_count = 0;
				while let Some(event) = upstream.next().await {
					event_count += 1;
					eprintln!("[STREAM] Event #{}: {:?}", event_count, &event);
					match event {
						StreamEvent::TextDelta { content } => {
							if let Some(start) = reasoning_start.take() {
								reasoning_latency_ms = Some(start.elapsed().as_millis() as i32);
							}
							full_content.push_str(&content);
							iteration_content.push_str(&content);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::TextDelta { content }).unwrap_or_default()
							));
						}
						StreamEvent::ReasoningDelta { content } => {
							if reasoning_start.is_none() {
								reasoning_start = Some(Instant::now());
							}
							reasoning_content.push_str(&content);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ReasoningDelta { content }).unwrap_or_default()
							));
						}
						StreamEvent::ToolCallStart { id, name, args_json } => {
							eprintln!("[STREAM] Tool call start: {} ({})", name, id);
							pending_tool_calls.insert(id.clone(), (name.clone(), args_json.to_string()));
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ToolCallStart { id, name }).unwrap_or_default()
							));
						}
						StreamEvent::ToolCallDelta { id, args_delta_json } => {
							if let Some((_, args)) = pending_tool_calls.get_mut(&id) {
								args.push_str(&args_delta_json.to_string());
							}
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ToolCallDelta {
									id,
									args_delta: args_delta_json.to_string()
								}).unwrap_or_default()
							));
						}
						StreamEvent::ToolCallEnd { id } => {
							eprintln!("[STREAM] Tool call end: {}", id);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ToolCallEnd { id: id.clone() }).unwrap_or_default()
							));

							if let Some((tool_name, args_str)) = pending_tool_calls.remove(&id) {
								let args: serde_json::Value = serde_json::from_str(&args_str).unwrap_or_default();
								eprintln!("[STREAM] Executing tool: {} with args: {:?}", tool_name, args);

								let exec_start = Instant::now();
								let tool_result = execute_tool_by_name(&db, user.id, &tool_name, args.clone()).await;
								let execution_ms = exec_start.elapsed().as_millis() as i32;

								let (output, error, tool_id, tool_function) = match tool_result {
									Ok(result) => (result.output, None, Some(result.tool_id), result.function_id),
									Err(e) => (serde_json::json!({"error": e}), Some(e), None, None),
								};

								tool_results.push((id.clone(), tool_name.clone(), args, output.clone(), error.clone(), execution_ms, tool_id, tool_function));

								yield Ok::<_, Infallible>(Event::default().data(
									serde_json::to_string(&StreamData::ToolResult { id, output, error, tool_id, tool_function, tool_name: Some(tool_name) }).unwrap_or_default()
								));
							}
						}
						StreamEvent::Tokens { input, output } => {
							input_tokens += input;
							output_tokens += output;
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::Tokens {
									input: input_tokens,
									output: output_tokens,
									reasoning: if reasoning_tokens > 0 { Some(reasoning_tokens) } else { None },
								}).unwrap_or_default()
							));
						}
						StreamEvent::OpenAIMetadata { completion_tokens_details, .. } => {
							if let Some(details) = completion_tokens_details {
								if details.reasoning_tokens > 0 {
									reasoning_tokens += details.reasoning_tokens;
								}
							}
						}
						StreamEvent::Error { code, message } => {
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default()
							));
						}
						StreamEvent::Done => {
							eprintln!("[STREAM] Stream done, tool_results: {}", tool_results.len());
							if tool_results.is_empty() {
								let latency_ms = start_time.elapsed().as_millis() as i32;

								let usage_details = crate::types::UsageDetails {
									input_tokens: Some(input_tokens as i32),
									output_tokens: Some(output_tokens as i32),
									reasoning_tokens: Some(reasoning_tokens as i32),
									latency_ms: Some(latency_ms),
									reasoning_latency_ms,
								};

								let reasoning_details = crate::types::ReasoningDetails {
									effort: reasoning_effort.clone(),
									budget_tokens: match reasoning_budget_tokens {
										Some(b) => Some(b as i32),
										None => None,
									},
								};

								let cost_details = crate::types::CostDetails::default();

								let message = sqlx::query_as::<_, Message>(
									r#"
										INSERT INTO messages (
											chat_id, role, content, reasoning_content,
											model_id, reasoning_details, usage_details, cost_details
										)
										VALUES ($1, 'assistant', $2, $3, $4, $5, $6, $7)
										RETURNING *
										"#,
								)
								.bind(chat_id)
								.bind(&full_content)
								.bind(if reasoning_content.is_empty() { None } else { Some(&reasoning_content) })
								.bind(&model.id)
								.bind(sqlx::types::Json(reasoning_details))
								.bind(sqlx::types::Json(usage_details))
								.bind(sqlx::types::Json(cost_details))
								.fetch_one(&db)
								.await;

								let _ = sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
									.bind(chat_id)
									.execute(&db)
									.await;

								match message {
									Ok(msg) => {
									if !all_tool_executions.is_empty() {
										for (call_id, _tool_name, args, output, error, exec_ms, tool_id, function_id) in &all_tool_executions {
											let _ = sqlx::query(
												"INSERT INTO tool_executions (message_id, tool_call_id, input_args, output, error, execution_ms, tool_id, tool_function) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
											)
											.bind(msg.id)
											.bind(call_id)
											.bind(args)
											.bind(output)
											.bind(error)
											.bind(exec_ms)
											.bind(tool_id)
											.bind(function_id)
											.execute(&db)
											.await;
										}
										eprintln!("[STREAM] Saved {} tool executions for message {}", all_tool_executions.len(), msg.id);
									}

									let message_response: ChatMessageResponse = msg.into();
									yield Ok::<_, Infallible>(Event::default().data(
										serde_json::to_string(&StreamData::Done { message: message_response }).unwrap_or_default()
										));
									}
									Err(e) => {
										eprintln!("[STREAM] Failed to save message: {e}");
										yield Ok::<_, Infallible>(Event::default().data(
											serde_json::to_string(&StreamData::Error {
												code: "save_failed".to_string(),
												message: "Failed to save response".to_string(),
											}).unwrap_or_default()
										));
									}
								}
								break 'agentic_loop;
							} else {
								eprintln!("[STREAM] Continuing agentic loop with {} tool results", tool_results.len());

								if !iteration_content.is_empty() {
									current_messages.push(OmniMessage {
										role: Role::Assistant,
										parts: vec![ContentPart::Text(iteration_content.clone())],
										name: None,
									});
								}

								for (call_id, tool_name, args, output, error, exec_ms, tool_id, function_id) in tool_results.drain(..) {
									let result_text = if let Some(ref err) = error {
										format!("Error: {}", err)
									} else {
										serde_json::to_string(&output).unwrap_or_else(|_| output.to_string())
									};

									current_messages.push(OmniMessage {
										role: Role::Tool,
										parts: vec![ContentPart::Text(result_text)],
										name: Some(format!("{}:{}", tool_name, call_id)),
									});

									all_tool_executions.push((call_id, tool_name, args, output, error, exec_ms, tool_id, function_id));
								}

								continue 'agentic_loop;
							}
						}
						_ => {}
					}
				}
				eprintln!("[STREAM] Stream ended after {} events", event_count);
				break 'agentic_loop;
		}
	};

	Sse::new(sse_stream).keep_alive(KeepAlive::default()).into_response()
}
