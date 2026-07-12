//! Streaming routes for AI responses.
//!
//! Server-Sent Events (SSE) endpoint for streaming AI chat completions.

use crate::ai;
use crate::routes::public::auth::get_current_user;
use crate::types::models::{Model, ModelPricing};
use crate::types::models_configs::{ModelConfig, ModelConfigViewer};
use crate::types::{
	Budget, Chat, ChatMessageResponse, ClientToolResultRequest, Message, MessagePart, RequestSettings, StreamData, StreamRequest, StreamingAssistantMessageCreate,
	StreamingUserMessageCreate, Tool, ToolExecution, ToolExecutionInternal, ToolExecutionResponse, ToolFunction, UsageEvent, UsageEventRecord, UserToolSettings,
};
use crate::types::{CostDetails, JobState};
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
use rust_decimal::{Decimal, prelude::FromPrimitive};
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Instant};
use tower_cookies::Cookies;
use uuid::Uuid;

fn error_stream(code: impl Into<String>, message: impl Into<String>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
	let code = code.into();
	let message = message.into();
	let data = serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default();
	Sse::new(futures_util::stream::once(async move { Ok::<_, Infallible>(Event::default().data(data)) })).keep_alive(KeepAlive::default())
}

async fn acquire_budget_lock(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
	let mut tx = pool.begin().await?;
	sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text, 0))")
		.bind(user_id)
		.execute(&mut *tx)
		.await?;
	Ok(tx)
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
fn interpolate_system_prompt(template: &str, user: &crate::types::User, model: &Model) -> String {
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
fn merge_sampling_with_priority(request_sampling: Option<&Sampling>, model_config: Option<&ModelConfig>) -> Sampling {
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
			.or_else(|| model_config.and_then(|mc| mc.max_output_tokens.map(|t| t as u32))),
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

async fn resolve_tools(db: &sqlx::PgPool, user_id: Uuid, enabled_tool_ids: &[String]) -> (Vec<ToolSpec>, ToolChoice, Vec<String>) {
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
			let label = if tool.display_name.trim().is_empty() { tool.name.as_str() } else { tool.display_name.trim() };
			system_prompts.push(format!("Instructions for the \"{label}\" tool:\n{prompt}"));
		}
		let funcs = functions_by_tool.get(&tool.id).map(|v| v.as_slice()).unwrap_or(&[]);
		tool_specs.extend(tool.to_tool_specs(funcs));
	}

	if tool_specs.is_empty() {
		(vec![], ToolChoice::None, system_prompts)
	} else {
		(tool_specs, ToolChoice::Auto, system_prompts)
	}
}

/// Returns `(mcp_server_id, mcp_tool_name)` if the given tool name resolves to a
/// user-owned MCP tool that must be executed client-side. Returns `None` for
/// system/global tools and all non-MCP tool types.
async fn get_client_tool_info(db: &sqlx::PgPool, tool_name: &str, user_id: Uuid) -> Option<(uuid::Uuid, String, uuid::Uuid)> {
	use crate::types::tools::{McpSourceConfig, ToolSourceKind};

	let tool = Tool::find_enabled_by_name_for_user(db, tool_name, &user_id).await.ok().flatten()?;

	if tool.source_kind != ToolSourceKind::Mcp || tool.owner_id.is_none() {
		return None;
	}

	let config: McpSourceConfig = serde_json::from_value(tool.source_config).ok()?;
	Some((config.mcp_server_id, config.tool_name, tool.id))
}

async fn execute_tool_by_name(
	db: &sqlx::PgPool,
	mcp_pool: &crate::utils::tools::McpConnectionPool,
	user_id: Uuid,
	full_tool_name: &str,
	input: serde_json::Value,
) -> Result<crate::types::ToolExecutionResult, String> {
	use crate::types::ToolSourceKind;

	let mut tool: Option<Tool>;
	let mut function_name: Option<String> = None;
	let mut function_id: Option<Uuid> = None;

	tool = Tool::find_enabled_by_name_for_user(db, full_tool_name, &user_id)
		.await
		.map_err(|e| format!("Database error: {e}"))?;

	if tool.is_none() && full_tool_name.contains('_') {
		let underscores: Vec<_> = full_tool_name.match_indices('_').collect();
		for (pos, _) in underscores.iter().rev() {
			let potential_tool_name = &full_tool_name[..*pos];
			let potential_func_name = &full_tool_name[pos + 1..];

			let found = Tool::find_enabled_by_name_for_user(db, potential_tool_name, &user_id)
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
		let func = ToolFunction::find_by_tool_and_name(db, &tool.id, fn_name)
			.await
			.map_err(|e| format!("Database error: {e}"))?
			.ok_or_else(|| format!("Function '{}' not found in tool '{}'", fn_name, tool.name))?;
		function_id = Some(func.id);
		func.entrypoint
	} else {
		None
	};

	let settings = UserToolSettings::find_effective_for_user(db, &tool.id, &user_id)
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
		ToolSourceKind::Mcp => {
			let output = execute_mcp_tool(db, mcp_pool, user_id, &tool, input).await.map_err(|e| format!("{e}"))?;
			Ok(crate::types::ToolExecutionResult {
				tool_id: tool.id,
				function_id,
				output,
			})
		}
		ToolSourceKind::Wasm => Err(format!("WASM tools are not supported in chat execution: {:?}", tool.source_kind)),
	}
}

/// Execute an MCP-backed tool by resolving its server, obtaining a pooled client,
/// and calling the remote tool. On transport failure the cached client is evicted
/// so the next call reconnects.
async fn execute_mcp_tool(
	db: &sqlx::PgPool,
	mcp_pool: &crate::utils::tools::McpConnectionPool,
	user_id: Uuid,
	tool: &Tool,
	input: serde_json::Value,
) -> Result<serde_json::Value, crate::utils::tools::ToolError> {
	use crate::types::McpSourceConfig;
	use crate::types::tools::McpServer;
	use crate::utils::tools::ToolError;

	let config: McpSourceConfig = serde_json::from_value(tool.source_config.clone()).map_err(|e| ToolError::McpError(format!("Invalid MCP source config: {e}")))?;

	let server = McpServer::find_owned_or_system(db, &config.mcp_server_id, &user_id)
		.await
		.map_err(|e| ToolError::McpError(format!("Database error: {e}")))?
		.ok_or_else(|| ToolError::NotFound(format!("MCP server {} not found", config.mcp_server_id)))?;

	if !server.is_enabled {
		return Err(ToolError::ExecutionFailed("MCP server is disabled".to_string()));
	}

	let client = server.get_client(mcp_pool).await?;
	match client.call_tool(&config.tool_name, input).await {
		Ok(output) => Ok(output),
		Err(e) => {
			mcp_pool.evict(&server.id).await;
			Err(e)
		}
	}
}

/// Hydrate a stored image into content parts. `generated` distinguishes an assistant-
/// generated image (gets an editable `image_id` handle) from a user-uploaded one.
async fn hydrate_image(db: &sqlx::PgPool, image_id: &str, vision: bool, generated: bool) -> Vec<ContentPart> {
	let Ok(uuid) = uuid::Uuid::parse_str(image_id) else {
		return Vec::new();
	};
	let mut parts = Vec::new();
	if vision {
		if let Ok(Some((data, mime))) = crate::utils::images::get_image(db, uuid).await {
			use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
			let data_uri = format!("data:{};base64,{}", mime, BASE64.encode(&data));
			parts.push(ContentPart::ImageUrl { url: data_uri, mime: Some(mime) });
		}
		if generated {
			parts.push(ContentPart::Text(format!(
				"[The image above was generated earlier (image_id: {image_id}) and is already displayed to the user. To modify it, call imagegen_edit with image_id \"{image_id}\".]"
			)));
		}
	} else if generated {
		let caption = crate::utils::images::image_caption(db, uuid).await.ok().flatten();
		let text = match caption {
			Some(c) if !c.trim().is_empty() => format!(
				"[An image was generated earlier (image_id: {image_id}) from the prompt: \"{}\". It is already displayed to the user. To modify it, call imagegen_edit with image_id \"{image_id}\".]",
				c.trim()
			),
			_ => format!("[An image was generated earlier (image_id: {image_id}) and is already displayed to the user. To modify it, call imagegen_edit with image_id \"{image_id}\".]"),
		};
		parts.push(ContentPart::Text(text));
	} else {
		parts.push(ContentPart::Text(format!("[User-provided image {image_id} is not visible to this model.]")));
	}
	parts
}

/// Build the `tool` role message for a given tool call, pulling its result from the
/// stored executions (or a placeholder if it was never recorded).
fn tool_message(execs: Option<&Vec<ToolExecutionResponse>>, call_names: &HashMap<String, String>, call_id: &str) -> OmniMessage {
	let exec = execs.and_then(|list| list.iter().find(|e| e.tool_call_id == call_id));
	let content = match exec {
		Some(e) if e.error.is_some() => format!("Error: {}", e.error.as_deref().unwrap_or_default()),
		Some(e) => match &e.output {
			Some(output) => serde_json::to_string(output).unwrap_or_else(|_| output.to_string()),
			None => "{}".to_string(),
		},
		None => "{\"error\":\"tool result was not recorded\"}".to_string(),
	};
	let name = call_names.get(call_id).map(String::as_str).unwrap_or("tool");
	OmniMessage {
		role: Role::Tool,
		parts: vec![ContentPart::Text(content)],
		name: Some(format!("{name}:{call_id}")),
	}
}

async fn get_omni_messages(db: &sqlx::PgPool, messages: Vec<Message>, vision: bool) -> Vec<OmniMessage> {
	let assistant_ids: Vec<uuid::Uuid> = messages.iter().filter(|m| m.role == "assistant").map(|m| m.id).collect();
	let executions = Message::tool_executions_for_messages(db, &assistant_ids).await.unwrap_or_default();

	let mut result = Vec::with_capacity(messages.len());
	for m in messages.into_iter().filter(|m| m.role == "user" || m.role == "assistant") {
		let message_id = m.id;
		let is_assistant = m.role == "assistant";

		let Some(stored_parts) = m.content_parts.and_then(|json| serde_json::from_value::<Vec<MessagePart>>(json).ok()) else {
			result.push(OmniMessage {
				role: if is_assistant { Role::Assistant } else { Role::User },
				parts: vec![ContentPart::Text(m.content)],
				name: None,
			});
			continue;
		};

		if !is_assistant {
			let mut parts = Vec::new();
			for part in stored_parts {
				match part {
					MessagePart::Text { text } => parts.push(ContentPart::Text(text)),
					MessagePart::Image { image_id } => parts.extend(hydrate_image(db, &image_id, vision, false).await),
					_ => {}
				}
			}
			if parts.is_empty() {
				parts.push(ContentPart::Text(m.content));
			}
			result.push(OmniMessage { role: Role::User, parts, name: None });
			continue;
		}

		// Assistant turn: walk parts in stored order and flush an assistant message at each
		// tool-result boundary, so a sequence of retries (call -> error -> call -> success)
		// replays as separate steps instead of collapsing into one parallel batch.
		let execs = executions.get(&message_id);
		let mut call_names: HashMap<String, String> = HashMap::new();
		let mut unresolved_calls: Vec<String> = Vec::new();
		let mut buffer: Vec<ContentPart> = Vec::new();

		for part in stored_parts {
			match part {
				MessagePart::Text { text } => buffer.push(ContentPart::Text(text)),
				MessagePart::Reasoning { .. } => {}
				MessagePart::Image { image_id } => buffer.extend(hydrate_image(db, &image_id, vision, true).await),
				MessagePart::ToolCall { id, name, arguments } => {
					call_names.insert(id.clone(), name.clone());
					unresolved_calls.push(id.clone());
					buffer.push(ContentPart::ToolCall { id, name, arguments });
				}
				MessagePart::ToolResult { tool_call_id } => {
					if !buffer.is_empty() {
						result.push(OmniMessage { role: Role::Assistant, parts: std::mem::take(&mut buffer), name: None });
					}
					unresolved_calls.retain(|c| c != &tool_call_id);
					result.push(tool_message(execs, &call_names, &tool_call_id));
				}
			}
		}

		if !buffer.is_empty() {
			result.push(OmniMessage { role: Role::Assistant, parts: buffer, name: None });
		}
		// Legacy messages (saved before tool-result markers) and any call whose result marker
		// is missing: emit their `tool` messages here so no assistant tool_call is left dangling.
		for call_id in unresolved_calls {
			result.push(tool_message(execs, &call_names, &call_id));
		}
	}
	result
}

pub async fn stream_completion(State(state): State<Arc<JobState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<StreamRequest>) -> impl IntoResponse {
	eprintln!("[STREAM] Starting stream completion");

	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return error_stream("not_authenticated", "Authentication required").into_response();
	};

	let chat = Chat::find_by_id_and_user(&state.db, &chat_id, &user.id).await;

	let _chat = match chat {
		Ok(Some(chat)) => chat,
		Ok(None) => return error_stream("not_found", "Chat not found").into_response(),
		Err(e) => {
			eprintln!("[STREAM] Failed to fetch chat: {e}");
			return error_stream("internal_error", "Failed to fetch chat").into_response();
		}
	};

	let model = Model::find_by_model_id(&state.db, &req.model_key).await;

	let model = match model {
		Ok(Some(m)) => m,
		Ok(None) => return error_stream("not_found", "Model not found").into_response(),
		Err(e) => {
			eprintln!("[STREAM] Failed to fetch model: {e}");
			return error_stream("internal_error", "Failed to fetch model").into_response();
		}
	};

	match Model::can_user_use_model(&state.db, &user.id, &model.id).await {
		Ok(true) => {}
		Ok(false) => return error_stream("model_not_allowed", "Model not available for this user").into_response(),
		Err(e) => {
			eprintln!("[STREAM] Failed to check model access: {e}");
			return error_stream("internal_error", "Failed to check model access").into_response();
		}
	}

	let mut budget_lock = None;
	match ModelPricing::is_free(&state.db, &model.id).await {
		Ok(true) => {}
		Ok(false) => {
			let lock = match acquire_budget_lock(&state.db, &user.id).await {
				Ok(lock) => lock,
				Err(e) => {
					eprintln!("[STREAM] Failed to acquire budget lock: {e}");
					return error_stream("internal_error", "Failed to check budget status").into_response();
				}
			};
			match Budget::status_for_user(&state.db, &user.id).await {
				Ok(status) if status.blocked_model_ids.contains(&model.id) => {
					return error_stream("budget_exceeded", "Budget exceeded for this model").into_response();
				}
				Ok(_) => {
					budget_lock = Some(lock);
				}
				Err(e) => {
					eprintln!("[STREAM] Failed to check budget status: {e}");
					return error_stream("internal_error", "Failed to check budget status").into_response();
				}
			}
		}
		Err(e) => {
			eprintln!("[STREAM] Failed to check model pricing: {e}");
			return error_stream("internal_error", "Failed to check model pricing").into_response();
		}
	}

	let user_model_config = ModelConfig::find_for_user_by_stable_key(&state.db, ModelConfigViewer { user_id: &user.id }, &req.model_key)
		.await
		.ok()
		.flatten();

	let system_model_config = ModelConfig::find_system_by_stable_key(&state.db, &req.model_key).await.ok().flatten();

	let reasoning_details = crate::types::ReasoningDetails {
		effort: req.reasoning_effort.clone(),
		budget_tokens: req.reasoning_budget_tokens.map(|b| b as i32),
	};
	let usage_details = crate::types::UsageDetails::default();
	let cost_details = crate::types::CostDetails::default();
	let request_settings = RequestSettings {
		model_key: Some(req.model_key.clone()),
		provider_slug: req.provider_slug.as_ref().map(|slug| slug.trim().to_string()).filter(|slug| !slug.is_empty()),
		provider_routing_mode: Some(req.provider_routing_mode.clone().unwrap_or_else(|| "prefer".to_string())),
		enabled_tools: req.tools_enabled.clone().unwrap_or_default(),
	};

	let content_parts_json = req.parts.as_ref().map(|parts| serde_json::to_value(parts).ok()).flatten();

	// Get the last active message to use as parent for the new user message
	let last_active_message_id = Message::last_active_id(&state.db, &chat_id).await.ok().flatten();

	// Only create user message if not regenerating/skipping
	let user_message: Option<Message> = if !req.skip_user_message {
		let msg = Message::create_streaming_user(
			&state.db,
			StreamingUserMessageCreate {
				chat_id: &chat_id,
				content: &req.content,
				content_parts: content_parts_json.as_ref(),
				model_id: &model.id,
				reasoning_details: &reasoning_details,
				usage_details: &usage_details,
				cost_details: &cost_details,
				request_settings: &request_settings,
				parent_id: last_active_message_id,
			},
		)
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
	let messages = Message::list_active_by_chat(&state.db, &chat_id).await;

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
		let original_msg = Message::find_by_id_and_chat(&state.db, &regen_uuid, &chat_id).await;

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
		let next_fork_index = Message::next_assistant_fork_index(&state.db, &chat_id, parent_id).await.unwrap_or(1);

		// Deactivate the old assistant message and all its descendants
		let _ = Message::deactivate_active_assistant_forks(&state.db, &chat_id, parent_id).await;

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

	let vision = omni_model.input_modalities.iter().any(|m| matches!(m, omniference::types::Modality::Image));
	let omni_messages = get_omni_messages(&state.db, messages, vision).await;
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
	ir.sampling = merge_sampling_with_priority(req.sampling.as_ref(), user_model_config.as_ref());
	ir.reasoning = merge_reasoning_with_priority(req.reasoning_effort.as_ref(), req.reasoning_budget_tokens, system_model_config.as_ref());

	// Apply the user-picked upstream provider (OpenRouter routing), only when the instance owner
	// has enabled the provider selector and the model's provider is an OpenRouter gateway.
	if crate::config::Config::get().enable_provider_selector() {
		if let Some(slug) = req.provider_slug.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
			if ir.model.provider.endpoint.kind == omniference::types::ProviderKind::OpenRouter {
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
	}

	let mut tool_system_prompts: Vec<String> = Vec::new();
	if let Some(ref enabled_tool_ids) = req.tools_enabled {
		let (specs, choice, prompts) = resolve_tools(&state.db, user.id, enabled_tool_ids).await;
		if !specs.is_empty() {
			ir.tools = specs;
			ir.tool_choice = choice;
			tool_system_prompts = prompts;
		}
	}

	let mut system_segments: Vec<String> = Vec::new();
	let system_prompt_text = system_model_config.as_ref().and_then(|mc| mc.system_prompt.as_deref()).unwrap_or("");
	if !system_prompt_text.is_empty() {
		system_segments.push(interpolate_system_prompt(system_prompt_text, &user, &model));
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

	let omni_messages_for_stream = ir.messages.clone();
	let ir_for_stream = ir.clone();

	let db = state.db.clone();
	let mcp_pool = state.mcp_pool.clone();
	let client_tool_pending = state.client_tool_pending.clone();
	let start_time = Instant::now();
	let reasoning_effort = req.reasoning_effort.clone();
	let reasoning_budget_tokens = req.reasoning_budget_tokens;
	let stream_request_settings = request_settings.clone();
	let mut budget_lock = budget_lock;

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
	let mut cost_details = CostDetails::default();

	let mut current_messages = omni_messages_for_stream;
	let mut iteration = 0;
	const MAX_ITERATIONS: usize = 10;

	let mut all_tool_executions: Vec<ToolExecutionInternal> = Vec::new();
	let mut assistant_message_parts: Vec<MessagePart> = Vec::new();
	let request_settings = stream_request_settings;

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

				for (i, m) in base_ir.messages.iter().enumerate() {
					let role = match m.role {
						Role::System => "system",
						Role::User => "user",
						Role::Assistant => "assistant",
						Role::Tool => "tool",
						Role::Developer => "developer",
					};
					let tool_calls: Vec<&str> = m
						.parts
						.iter()
						.filter_map(|p| match p {
							ContentPart::ToolCall { id, .. } => Some(id.as_str()),
							_ => None,
						})
						.collect();
					eprintln!("[STREAM] msg[{i}] role={role} name={:?} tool_calls={:?}", m.name, tool_calls);
				}

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
				let mut iteration_reasoning = String::new();
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
							iteration_reasoning.push_str(&content);
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

								let exec_start = Instant::now();

								let client_info = get_client_tool_info(&db, &tool_name, user.id).await;

								let (output, error, tool_id, function_id) = if let Some((mcp_server_id, mcp_tool_name, actual_tool_id)) = client_info {
									eprintln!("[STREAM] Delegating tool {} to client (mcp_server_id={})", tool_name, mcp_server_id);

									yield Ok::<_, Infallible>(Event::default().data(
										serde_json::to_string(&StreamData::ClientToolCall {
											id: id.clone(),
											name: tool_name.clone(),
											args: args.clone(),
											mcp_server_id,
											mcp_tool_name,
										}).unwrap_or_default()
									));

									let rx = client_tool_pending.register(id.clone(), user.id).await;

									let result = match tokio::time::timeout(
										std::time::Duration::from_secs(120),
										rx,
									).await {
										Ok(Ok(r)) => r,
										_ => {
											client_tool_pending.cancel(&id, user.id).await;
											eprintln!("[STREAM] Client tool call {} timed out or failed", id);
											serde_json::json!({"error": "Client tool execution timed out"})
										}
									};

									(result, None, Some(actual_tool_id), None)
								} else {
									eprintln!("[STREAM] Executing tool server-side: {} with args: {:?}", tool_name, args);
									let tool_result = execute_tool_by_name(&db, &mcp_pool, user.id, &tool_name, args.clone()).await;
									match tool_result {
										Ok(result) => (result.output, None, Some(result.tool_id), result.function_id),
										Err(e) => (serde_json::json!({"error": e}), Some(e), None, None),
									}
								};

								let execution_ms = exec_start.elapsed().as_millis() as i32;

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
						StreamEvent::Cost { cost } => {
							cost_details.input = cost.prompt;
							cost_details.output = cost.completion;
							cost_details.reasoning = cost.reasoning;
							cost_details.total = Some(cost.total);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::Usage { cost_details }).unwrap_or_default()
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

								let final_cost_total = if let Some(provider_total) = cost_details.total {
									Decimal::from_f64(provider_total).unwrap_or(Decimal::ZERO)
								} else {
									match ModelPricing::usage_cost(&db, &model.id, input_tokens as i32, output_tokens as i32, reasoning_tokens as i32).await {
										Ok(Some(cost)) => cost,
										Ok(None) => Decimal::ZERO,
										Err(e) => {
											eprintln!("[STREAM] Failed to compute local usage cost for model {}: {e}", model.id);
											Decimal::ZERO
										}
									}
								};

									if !iteration_reasoning.is_empty() {
										assistant_message_parts.push(MessagePart::Reasoning { text: iteration_reasoning.clone() });
									}
									if !iteration_content.is_empty() {
										assistant_message_parts.push(MessagePart::Text { text: iteration_content.clone() });
									}
									let has_tool_calls = assistant_message_parts.iter().any(|p| matches!(p, MessagePart::ToolCall { .. }));
									let content_parts_json = if has_tool_calls {
										serde_json::to_value(&assistant_message_parts).ok()
									} else {
										None
									};

									let message = Message::create_streaming_assistant(
										&db,
										StreamingAssistantMessageCreate {
											chat_id: &chat_id,
											content: &full_content,
											content_parts: content_parts_json.as_ref(),
											reasoning_content: if reasoning_content.is_empty() { None } else { Some(&reasoning_content) },
											model_id: &model.id,
											reasoning_details: &reasoning_details,
											usage_details: &usage_details,
											cost_details: &cost_details,
											request_settings: &request_settings,
											parent_id: assistant_parent_id,
											fork_index: assistant_fork_index,
										},
									)
									.await;

									match message {
										Ok(msg) => {
											let usage_recorded = UsageEvent::record(&db, UsageEventRecord {
												user_id: &user.id,
												team_id: Budget::primary_team_id(&db, &user.id).await.ok().flatten(),
												model_id: &model.id,
												provider_id: &model.provider_id,
												request_type: "chat",
												input_tokens: input_tokens as i32,
												output_tokens: output_tokens as i32,
												reasoning_tokens: reasoning_tokens as i32,
												cost_total: final_cost_total,
											}).await;
											match usage_recorded {
												Ok(_) => {
													if let Some(lock) = budget_lock.take() {
														if let Err(e) = lock.commit().await {
															eprintln!("[STREAM] Failed to release budget lock for message {}: {e}", msg.id);
														}
													}
												}
												Err(e) => {
													eprintln!("[STREAM] Failed to record usage event for message {}: {e}", msg.id);
													yield Ok::<_, Infallible>(Event::default().data(
														serde_json::to_string(&StreamData::Error {
															code: "accounting_failed".to_string(),
															message: "Failed to record usage".to_string(),
														}).unwrap_or_default()
													));
													break 'agentic_loop;
												}
											}
											if !all_tool_executions.is_empty() {
												if let Err(e) = ToolExecution::create_for_message_batch(&db, &msg.id, &all_tool_executions).await {
													eprintln!("[STREAM] Failed to save tool executions for message {}: {e}", msg.id);
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
								if !iteration_reasoning.is_empty() {
									assistant_message_parts.push(MessagePart::Reasoning { text: iteration_reasoning.clone() });
								}
								if !iteration_content.is_empty() {
									assistant_parts.push(ContentPart::Text(iteration_content.clone()));
									assistant_message_parts.push(MessagePart::Text { text: iteration_content.clone() });
								}
								for (call_id, tool_name, args) in completed_tool_calls.drain(..) {
									let arguments = match &args {
										serde_json::Value::String(s) => s.clone(),
										other => other.to_string(),
									};
									assistant_message_parts.push(MessagePart::ToolCall {
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
									current_messages.push(OmniMessage {
										role: Role::Assistant,
										parts: assistant_parts,
										name: None,
									});
								}

								for exec in tool_results.drain(..) {
									if matches!(exec.tool_name.as_str(), "imagegen" | "imagegen_generate" | "imagegen_edit")
										&& exec.error.is_none()
										&& let Some(image_id) = exec.output.get("image_id").and_then(serde_json::Value::as_str)
										&& Uuid::parse_str(image_id).is_ok()
									{
										assistant_message_parts.push(MessagePart::Image { image_id: image_id.to_string() });
									}
									let result_text = if let Some(ref err) = exec.error {
										format!("Error: {}", err)
									} else {
										serde_json::to_string(&exec.output).unwrap_or_else(|_| exec.output.to_string())
									};

									eprintln!("[STREAM] Tool result -> role=tool name=\"{}:{}\" ({} bytes)", exec.tool_name, exec.call_id, result_text.len());
									current_messages.push(OmniMessage {
										role: Role::Tool,
										parts: vec![ContentPart::Text(result_text)],
										name: Some(format!("{}:{}", exec.tool_name, exec.call_id)),
									});
									assistant_message_parts.push(MessagePart::ToolResult { tool_call_id: exec.call_id.clone() });

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

/// POST `/api/v1/chats/{chat_id}/stream/tool-result`
///
/// Called by the browser after it has executed a client-side MCP tool call.
/// The result is forwarded to the waiting streaming loop via the oneshot channel
/// that was registered when the `ClientToolCall` SSE event was emitted.
pub async fn submit_client_tool_result(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(_chat_id): Path<Uuid>,
	Json(req): Json<ClientToolResultRequest>,
) -> impl IntoResponse {
	use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};

	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	if state.client_tool_pending.resolve(&req.call_id, user.id, req.result).await {
		ResponseBuilder::new(ResponseBody::Json(serde_json::json!({"ok": true}))).build()
	} else {
		ErrorBuilder::new(ErrorCode::NotFound).build()
	}
}
