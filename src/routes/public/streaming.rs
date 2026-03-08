//! Streaming routes for AI responses.
//!
//! Server-Sent Events (SSE) endpoint for streaming AI chat completions.

use crate::ai;
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::ai::ModelConfig;
use crate::types::{ChatMessageResponse, Message, MessagePart, StreamData, StreamRequest, Tool, ToolExecutionInternal, ToolFunction, UserToolSettings};
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
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Instant};
use tower_cookies::Cookies;
use uuid::Uuid;

fn error_stream(code: impl Into<String>, message: impl Into<String>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
	let code = code.into();
	let message = message.into();
	let data = serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default();
	Sse::new(futures_util::stream::once(async move { Ok::<_, Infallible>(Event::default().data(data)) })).keep_alive(KeepAlive::default())
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
fn interpolate_system_prompt(template: &str, user: &crate::types::User, model: &crate::types::AiModel) -> String {
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

//TODO: Fix SQL statement in terms of owner_id if owner_id is null, rn it is searched for user meaning it will not find any tools rn as they are global (owner_id == null)
async fn resolve_tools(db: &sqlx::PgPool, user_id: Uuid, enabled_tool_ids: &[String]) -> (Vec<ToolSpec>, ToolChoice) {
	if enabled_tool_ids.is_empty() {
		return (vec![], ToolChoice::None);
	}

	let tool_uuids: Vec<Uuid> = enabled_tool_ids.iter().filter_map(|id| Uuid::parse_str(id).ok()).collect();
	if tool_uuids.is_empty() {
		return (vec![], ToolChoice::None);
	}

	let tools = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE id = ANY($1) AND is_enabled = true AND (owner_id = $2 OR owner_id IS NULL)")
		.bind(&tool_uuids)
		.bind(user_id)
		.fetch_all(db)
		.await
		.unwrap_or_default();

	if tools.is_empty() {
		return (vec![], ToolChoice::None);
	}

	let tool_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();
	let functions: Vec<ToolFunction> = sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = ANY($1) ORDER BY sort_order, created_at")
		.bind(&tool_ids)
		.fetch_all(db)
		.await
		.unwrap_or_default();

	let mut functions_by_tool: HashMap<Uuid, Vec<ToolFunction>> = HashMap::new();
	for f in functions {
		functions_by_tool.entry(f.tool_id).or_default().push(f);
	}

	let mut tool_specs: Vec<ToolSpec> = vec![];
	for tool in tools {
		let funcs = functions_by_tool.get(&tool.id).map(|v| v.as_slice()).unwrap_or(&[]);
		tool_specs.extend(tool.to_tool_specs(funcs));
	}

	if tool_specs.is_empty() {
		(vec![], ToolChoice::None)
	} else {
		(tool_specs, ToolChoice::Auto)
	}
}

async fn execute_tool_by_name(db: &sqlx::PgPool, user_id: Uuid, full_tool_name: &str, input: serde_json::Value) -> Result<crate::types::ToolExecutionResult, String> {
	use crate::types::ToolSourceKind;

	let mut tool: Option<Tool>;
	let mut function_name: Option<String> = None;
	let mut function_id: Option<Uuid> = None;

	tool = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE name = $1 AND is_enabled = true AND owner_id = $2")
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

			let found = sqlx::query_as::<_, Tool>("SELECT * FROM tools WHERE name = $1 AND is_enabled = true")
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
		db: Some(std::sync::Arc::new(db.clone())),
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

async fn get_omni_messages(db: &sqlx::PgPool, messages: Vec<Message>) -> Vec<OmniMessage> {
	let mut result = Vec::with_capacity(messages.len());
	for m in messages.into_iter().filter(|m| m.role == "user" || m.role == "assistant") {
		let parts = if let Some(content_parts_json) = m.content_parts {
			if let Ok(stored_parts) = serde_json::from_value::<Vec<MessagePart>>(content_parts_json) {
				let mut omni_parts = Vec::new();
				for part in stored_parts {
					match part {
						MessagePart::Text { text } => {
							omni_parts.push(ContentPart::Text(text));
						}
						MessagePart::Image { image_id } => {
							if let Ok(uuid) = uuid::Uuid::parse_str(&image_id) {
								if let Ok(Some((data, mime))) = crate::utils::images::get_image(db, uuid).await {
									use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
									let b64 = BASE64.encode(&data);
									let data_uri = format!("data:{};base64,{}", mime, b64);
									omni_parts.push(ContentPart::ImageUrl { url: data_uri, mime: Some(mime) });
								}
							}
						}
					}
				}
				omni_parts
			} else {
				vec![ContentPart::Text(m.content)]
			}
		} else {
			vec![ContentPart::Text(m.content)]
		};

		result.push(OmniMessage {
			role: if m.role == "user" { Role::User } else { Role::Assistant },
			parts,
			name: None,
		});
	}
	result
}

pub async fn stream_completion(State(state): State<Arc<JobState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<StreamRequest>) -> impl IntoResponse {
	eprint!("[STREAM] Starting stream completion");

	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return error_stream("not_authenticated", "Authentication required").into_response();
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

	let user_model_config = sqlx::query_as::<_, ModelConfig>("SELECT * FROM model_configs WHERE owner_id = $1 AND stable_key = $2")
		.bind(user.id)
		.bind(&req.model_key)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	let system_model_config = sqlx::query_as::<_, ModelConfig>("SELECT * FROM model_configs WHERE owner_id IS NULL AND stable_key = $1")
		.bind(&req.model_key)
		.fetch_optional(&state.db)
		.await
		.ok()
		.flatten();

	let reasoning_details = crate::types::ReasoningDetails {
		effort: req.reasoning_effort.clone(),
		budget_tokens: req.reasoning_budget_tokens.map(|b| b as i32),
	};
	let usage_details = crate::types::UsageDetails::default();
	let cost_details = crate::types::CostDetails::default();

	let content_parts_json = req.parts.as_ref().map(|parts| serde_json::to_value(parts).ok()).flatten();

	// Get the last active message to use as parent for the new user message
	let last_active_message_id: Option<Uuid> =
		sqlx::query_scalar("SELECT id FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE ORDER BY created_at DESC LIMIT 1")
			.bind(chat_id)
			.fetch_optional(&state.db)
			.await
			.ok()
			.flatten();

	// Only create user message if not regenerating/skipping
	let user_message: Option<Message> = if !req.skip_user_message {
		let msg = sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, content_parts, model_id, reasoning_details, usage_details, cost_details, parent_id)
			VALUES ($1, 'user', $2, $3, $4, $5, $6, $7, $8)
			RETURNING *
			"#,
		)
		.bind(chat_id)
		.bind(&req.content)
		.bind(content_parts_json)
		.bind(model.id)
		.bind(sqlx::types::Json(reasoning_details))
		.bind(sqlx::types::Json(usage_details))
		.bind(sqlx::types::Json(cost_details))
		.bind(last_active_message_id)
		.fetch_one(&state.db)
		.await;

		match msg {
			Ok(m) => Some(m),
			Err(e) => {
				eprintln!("[STREAM] Failed to save user message: {e}");
				return error_stream("save_failed", "Failed to save message").into_response();
			}
		}
	} else {
		None
	};

	// Only fetch active fork messages for AI context
	let messages = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE ORDER BY created_at ASC")
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

	// Handle regeneration fork logic
	let (assistant_parent_id, assistant_fork_index, messages): (Option<Uuid>, i32, Vec<Message>) = if let Some(ref regen_id) = req.regenerate_from_message_id {
		// Parse the regenerate message ID
		let regen_uuid = match Uuid::parse_str(regen_id) {
			Ok(u) => u,
			Err(_) => return error_stream("invalid_request", "Invalid regenerate_from_message_id").into_response(),
		};

		// Fetch the original assistant message to get its parent_id
		let original_msg = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
			.bind(regen_uuid)
			.bind(chat_id)
			.fetch_optional(&state.db)
			.await;

		let original = match original_msg {
			Ok(Some(m)) => m,
			Ok(None) => return error_stream("not_found", "Original message not found").into_response(),
			Err(e) => {
				eprintln!("[STREAM] Failed to fetch original message: {e}");
				return error_stream("internal_error", "Failed to fetch original message").into_response();
			}
		};

		let parent_id = original.parent_id;

		// Get the next fork_index for siblings with same parent_id and role
		let next_fork_index: i32 = sqlx::query_scalar(
			r#"SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND role = 'assistant'"#,
		)
		.bind(chat_id)
		.bind(parent_id)
		.fetch_one(&state.db)
		.await
		.unwrap_or(1);

		// Deactivate the old assistant message and all its descendants
		let _ = sqlx::query(
			r#"
			WITH RECURSIVE descendants AS (
				SELECT id FROM messages 
				WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND role = 'assistant' AND is_active_fork = TRUE
				UNION ALL
				SELECT m.id FROM messages m
				INNER JOIN descendants d ON m.parent_id = d.id
				WHERE m.chat_id = $1
			)
			UPDATE messages SET is_active_fork = FALSE 
			WHERE id IN (SELECT id FROM descendants)
			"#,
		)
		.bind(chat_id)
		.bind(parent_id)
		.execute(&state.db)
		.await;

		// Filter messages to only include up to and including the parent_id
		// This prevents context from other fork branches being included
		let filtered_messages: Vec<Message> = if let Some(pid) = parent_id {
			let parent_idx = messages.iter().position(|m| m.id == pid);
			match parent_idx {
				Some(idx) => messages.into_iter().take(idx + 1).collect(),
				None => messages, // Parent not found in current list, use all
			}
		} else {
			// No parent (regenerating first assistant message), use empty
			vec![]
		};

		(parent_id, next_fork_index, filtered_messages)
	} else {
		// Normal flow: parent is the last user message or last message
		let parent = user_message.as_ref().map(|m| m.id).or_else(|| messages.last().map(|m| m.id));
		(parent, 1, messages)
	};

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

	let provider = match engine_read.get_provider(&omni_model.provider_name).await {
		Some(p) => p,
		None => {
			let msg = format!("Provider '{}' not found", omni_model.provider_name);
			drop(engine_read);
			return error_stream("provider_not_found", msg).into_response();
		}
	};

	let omni_messages = get_omni_messages(&state.db, messages).await;
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
	ir.sampling = merge_sampling_with_priority(req.sampling.as_ref(), user_model_config.as_ref(), &model);
	ir.reasoning = merge_reasoning_with_priority(req.reasoning_effort.as_ref(), req.reasoning_budget_tokens, system_model_config.as_ref());

	let system_prompt_text = system_model_config.as_ref().and_then(|mc| mc.system_prompt.as_deref()).unwrap_or("");
	if !system_prompt_text.is_empty() {
		let interpolated = interpolate_system_prompt(system_prompt_text, &user, &model);
		ir.messages.insert(
			0,
			OmniMessage {
				role: Role::System,
				parts: vec![ContentPart::Text(interpolated)],
				name: None,
			},
		);
	}

	if let Some(ref enabled_tool_ids) = req.tools_enabled {
		let (specs, choice) = resolve_tools(&state.db, user.id, enabled_tool_ids).await;
		if !specs.is_empty() {
			ir.tools = specs;
			ir.tool_choice = choice;
		}
	}

	let omni_messages_for_stream = ir.messages.clone();
	let ir_for_stream = ir.clone();

	let db = state.db.clone();
	let start_time = Instant::now();
	let reasoning_effort = req.reasoning_effort.clone();
	let reasoning_budget_tokens = req.reasoning_budget_tokens;

	let sse_stream = async_stream::stream! {
	eprintln!("[STREAM] Stream generator started");

	// Only emit user message saved event if we actually created one
	if let Some(ref msg) = user_message {
		let user_message_response: ChatMessageResponse = msg.clone().into();
		yield Ok::<_, Infallible>(Event::default().data(
			serde_json::to_string(&StreamData::UserMessageSaved { message: user_message_response }).unwrap_or_default()
		));
	}

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

	let mut all_tool_executions: Vec<ToolExecutionInternal> = Vec::new();

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
				let mut tool_results: Vec<ToolExecutionInternal> = Vec::new();
				let mut iteration_content = String::new();
				let mut completed_tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();

				let mut event_count = 0;
				while let Some(event) = upstream.next().await {
					event_count += 1;
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
							let delta_str = match &args_delta_json {
								serde_json::Value::String(s) => s.clone(),
								other => other.to_string(),
							};
							if let Some((_, args)) = pending_tool_calls.get_mut(&id) {
								args.push_str(&delta_str);
							}
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ToolCallDelta {
									id,
									args_delta: delta_str
								}).unwrap_or_default()
							));
						}
						StreamEvent::ToolCallEnd { id, args_json } => {
							eprintln!("[STREAM] Tool call end: {}", id);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::ToolCallEnd { id: id.clone() }).unwrap_or_default()
							));

							if let Some((tool_name, _)) = pending_tool_calls.remove(&id) {
								let args = args_json;
								completed_tool_calls.push((id.clone(), tool_name.clone(), args.clone()));
								eprintln!("[STREAM] Executing tool: {} with args: {:?}", tool_name, args);

								let exec_start = Instant::now();
								let tool_result = execute_tool_by_name(&db, user.id, &tool_name, args.clone()).await;
								let execution_ms = exec_start.elapsed().as_millis() as i32;

								let (output, error, tool_id, function_id) = match tool_result {
									Ok(result) => (result.output, None, Some(result.tool_id), result.function_id),
									Err(e) => (serde_json::json!({"error": e}), Some(e), None, None),
								};

								tool_results.push(ToolExecutionInternal {
									call_id: id.clone(),
									tool_name: tool_name.clone(),
									args,
									output: output.clone(),
									error: error.clone(),
									execution_ms,
									tool_id,
									function_id,
								});

								yield Ok::<_, Infallible>(Event::default().data(
									serde_json::to_string(&StreamData::ToolResult { id, output, error, tool_id, tool_function: function_id, tool_name: Some(tool_name) }).unwrap_or_default()
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
									budget_tokens: reasoning_budget_tokens.map(|b| b as i32),
								};

								let cost_details = crate::types::CostDetails::default();

								let message = sqlx::query_as::<_, Message>(
									r#"
										INSERT INTO messages (
											chat_id, role, content, reasoning_content,
											model_id, reasoning_details, usage_details, cost_details, parent_id, fork_index, is_active_fork
										)
										VALUES ($1, 'assistant', $2, $3, $4, $5, $6, $7, $8, $9, TRUE)
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
								.bind(assistant_parent_id)
							.bind(assistant_fork_index)
								.fetch_one(&db)
								.await;

								let _ = sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
									.bind(chat_id)
									.execute(&db)
									.await;

								match message {
									Ok(msg) => {
									if !all_tool_executions.is_empty() {
										for exec in &all_tool_executions {
											let _ = sqlx::query(
												"INSERT INTO tool_executions (message_id, tool_call_id, input_args, output, error, execution_ms, tool_id, tool_function) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
											)
											.bind(msg.id)
											.bind(&exec.call_id)
											.bind(&exec.args)
											.bind(&exec.output)
											.bind(&exec.error)
											.bind(exec.execution_ms)
											.bind(exec.tool_id)
											.bind(exec.function_id)
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

								let mut assistant_parts: Vec<ContentPart> = Vec::new();
								if !iteration_content.is_empty() {
									assistant_parts.push(ContentPart::Text(iteration_content.clone()));
								}
								for (call_id, tool_name, args) in completed_tool_calls.drain(..) {
									let arguments = match &args {
										serde_json::Value::String(s) => s.clone(),
										other => other.to_string(),
									};
									assistant_parts.push(ContentPart::ToolCall {
										id: call_id,
										name: tool_name,
										arguments,
									});
								}
								if !assistant_parts.is_empty() {
									current_messages.push(OmniMessage {
										role: Role::Assistant,
										parts: assistant_parts,
										name: None,
									});
								}

								for exec in tool_results.drain(..) {
									let result_text = if let Some(ref err) = exec.error {
										format!("Error: {}", err)
									} else {
										serde_json::to_string(&exec.output).unwrap_or_else(|_| exec.output.to_string())
									};

									current_messages.push(OmniMessage {
										role: Role::Tool,
										parts: vec![ContentPart::Text(result_text)],
										name: Some(exec.call_id.clone()),
									});

									all_tool_executions.push(exec);
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
