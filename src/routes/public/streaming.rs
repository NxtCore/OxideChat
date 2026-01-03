//! Streaming routes for AI responses.
//!
//! Server-Sent Events (SSE) endpoint for streaming AI chat completions.
//! This endpoint saves the user message first, then streams the AI response.

use crate::AppState;
use crate::ai;
use crate::routes::public::auth::get_current_user;
use crate::types::Message;
use axum::{
	Json,
	extract::{Path, State},
	response::{
		IntoResponse,
		sse::{Event, KeepAlive, Sse},
	},
};
use futures_util::StreamExt;
use omniference::{
	stream::StreamEvent,
	types::{ChatRequestIR, ContentPart, Message as OmniMessage, Role},
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::Instant};
use tower_cookies::Cookies;
use uuid::Uuid;

/// Request body for sending a message and streaming AI response
#[derive(Debug, Deserialize)]
pub struct StreamRequest {
	/// Message content from the user
	pub content: String,
	/// Model stable_key to use (e.g., "openai:gpt-4o")
	pub model_key: String,
	/// Reasoning effort (low, medium, high)
	pub reasoning_effort: Option<String>,
	/// Enabled tools
	pub tools_enabled: Option<Vec<String>>,
}

/// SSE event data
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamData {
	/// User message saved confirmation
	UserMessageSaved { message_id: Uuid },
	/// Text content delta
	TextDelta { content: String },
	/// Reasoning text delta (for models that support it)
	ReasoningDelta { content: String },
	/// Token count update
	Tokens { input: u32, output: u32, reasoning: Option<u32> },
	/// Error occurred
	Error { code: String, message: String },
	/// Stream completed with message info
	Done {
		message_id: Uuid,
		input_tokens: Option<i32>,
		output_tokens: Option<i32>,
		reasoning_tokens: Option<i32>,
		latency_ms: Option<i32>,
		reasoning_latency_ms: Option<i32>,
	},
}

/// Helper to create an error SSE stream
fn error_stream(code: impl Into<String>, message: impl Into<String>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
	let code = code.into();
	let message = message.into();
	let data = serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default();
	Sse::new(futures_util::stream::once(async move { Ok::<_, Infallible>(Event::default().data(data)) })).keep_alive(KeepAlive::default())
}

/// POST /api/v1/chats/:chat_id/stream
///
/// Send a message and stream the AI response.
/// This is a unified endpoint that:
/// 1. Saves the user message to the database
/// 2. Streams the AI response via SSE
/// 3. Saves the assistant message upon completion
pub async fn stream_completion(State(state): State<Arc<AppState>>, cookies: Cookies, Path(chat_id): Path<Uuid>, Json(req): Json<StreamRequest>) -> impl IntoResponse {
	// Authenticate user
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return error_stream("not_authenticated", "Authentication required").into_response(),
	};

	// Verify chat belongs to user
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

	// Save the user message first
	let user_message = sqlx::query_as::<_, Message>(
		r#"
		INSERT INTO messages (chat_id, role, content, model_id, reasoning_effort)
		VALUES ($1, 'user', $2, $3, $4)
		RETURNING *
		"#,
	)
	.bind(chat_id)
	.bind(&req.content)
	.bind(model.id)
	.bind(&req.reasoning_effort)
	.fetch_one(&state.db)
	.await;

	let user_message = match user_message {
		Ok(msg) => msg,
		Err(e) => {
			eprintln!("[STREAM] Failed to save user message: {e}");
			return error_stream("save_failed", "Failed to save message").into_response();
		}
	};

	// Fetch all chat messages for context
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

	// Get the AI engine and model
	let engine = ai::get();
	let engine_read = engine.read().await;

	// Get model by stable key
	let model = match engine_read.get_model(&req.model_key).await {
		Some(m) => m,
		None => {
			let msg = format!("Model '{}' not found", req.model_key);
			drop(engine_read);
			return error_stream("model_not_found", msg).into_response();
		}
	};

	let provider = match engine_read.get_provider(&model.provider_name).await {
		Some(p) => p,
		None => {
			let msg = format!("Provider '{}' not found", model.provider_name);
			drop(engine_read);
			return error_stream("provider_not_found", msg).into_response();
		}
	};

	// Build chat messages for omniference
	let omni_messages: Vec<OmniMessage> = messages
		.iter()
		.filter(|m| m.role == "user" || m.role == "assistant")
		.map(|m| OmniMessage {
			role: if m.role == "user" { Role::User } else { Role::Assistant },
			parts: vec![ContentPart::Text(m.content.clone())],
			name: None,
		})
		.collect();

	// Build the request using Default
	let mut ir = ChatRequestIR::default();
	ir.model.alias = model.id.clone();
	ir.model.model_id = model.id.clone();
	ir.model.provider = provider.endpoint.clone();
	ir.model.input_modalities = model.input_modalities.clone();
	ir.model.output_modalities = model.output_modalities.clone();
	ir.messages = omni_messages;
	ir.stream = true;
	ir.metadata.insert("user_id".to_string(), user.id.to_string());
	ir.metadata.insert("chat_id".to_string(), chat_id.to_string());

	// Execute the chat
	let stream_result = engine_read.chat(ir).await;
	drop(engine_read);

	match stream_result {
		Ok(upstream) => {
			let db = state.db.clone();
			let start_time = Instant::now();
			let user_msg_id = user_message.id;
			let model_key = req.model_key.clone();
			let reasoning_effort = req.reasoning_effort.clone();

			// Create the SSE stream
			let sse_stream = async_stream::stream! {
				// First, emit the user message saved event
				yield Ok::<_, Infallible>(Event::default().data(
					serde_json::to_string(&StreamData::UserMessageSaved { message_id: user_msg_id }).unwrap_or_default()
				));

				let mut full_content = String::new();
				let reasoning_content = String::new();
				let mut input_tokens: u32 = 0;
				let mut output_tokens: u32 = 0;
				let mut reasoning_tokens: u32 = 0;
				let mut reasoning_start: Option<Instant> = None;
				let mut reasoning_latency_ms: Option<i32> = None;

				tokio::pin!(upstream);

				while let Some(event) = upstream.next().await {
					match event {
						StreamEvent::TextDelta { content } => {
							// If we were tracking reasoning time, stop it now
							if let Some(start) = reasoning_start.take() {
								reasoning_latency_ms = Some(start.elapsed().as_millis() as i32);
							}
							full_content.push_str(&content);
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::TextDelta { content }).unwrap_or_default()
							));
						}
						StreamEvent::Tokens { input, output } => {
							input_tokens = input;
							output_tokens = output;
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::Tokens {
									input,
									output,
									reasoning: if reasoning_tokens > 0 { Some(reasoning_tokens) } else { None },
								}).unwrap_or_default()
							));
						}
						StreamEvent::OpenAIMetadata { completion_tokens_details, .. } => {
							// Extract reasoning tokens from OpenAI metadata
							if let Some(details) = completion_tokens_details {
								if details.reasoning_tokens > 0 {
									reasoning_tokens = details.reasoning_tokens;
								}
							}
						}
						StreamEvent::Error { code, message } => {
							yield Ok::<_, Infallible>(Event::default().data(
								serde_json::to_string(&StreamData::Error { code, message }).unwrap_or_default()
							));
						}
						StreamEvent::Done => {
							let latency_ms = start_time.elapsed().as_millis() as i32;

							// Save the assistant message
							let message = sqlx::query_as::<_, Message>(
								r#"
								INSERT INTO messages (
									chat_id, role, content, reasoning_content,
									model_id, reasoning_effort,
									input_tokens, output_tokens, reasoning_tokens,
									latency_ms, reasoning_latency_ms
								)
								VALUES ($1, 'assistant', $2, $3, $4, $5, $6, $7, $8, $9, $10)
								RETURNING *
								"#,
							)
							.bind(chat_id)
							.bind(&full_content)
							.bind(if reasoning_content.is_empty() { None } else { Some(&reasoning_content) })
							.bind(&model_key)
							.bind(&reasoning_effort)
							.bind(input_tokens as i32)
							.bind(output_tokens as i32)
							.bind(if reasoning_tokens > 0 { Some(reasoning_tokens as i32) } else { None::<i32> })
							.bind(latency_ms)
							.bind(reasoning_latency_ms)
							.fetch_one(&db)
							.await;

							// Update chat updated_at
							let _ = sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1")
								.bind(chat_id)
								.execute(&db)
								.await;

							match message {
								Ok(msg) => {
									yield Ok::<_, Infallible>(Event::default().data(
										serde_json::to_string(&StreamData::Done {
											message_id: msg.id,
											input_tokens: Some(input_tokens as i32),
											output_tokens: Some(output_tokens as i32),
											reasoning_tokens: if reasoning_tokens > 0 { Some(reasoning_tokens as i32) } else { None },
											latency_ms: Some(latency_ms),
											reasoning_latency_ms,
										}).unwrap_or_default()
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
						}
						_ => {}
					}
				}
			};

			Sse::new(sse_stream).keep_alive(KeepAlive::default()).into_response()
		}
		Err(e) => {
			eprintln!("[STREAM] Failed to start chat: {e}");
			error_stream("routing_error", e).into_response()
		}
	}
}
