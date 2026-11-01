use crate::types::{Message, MessagePart, ToolExecutionResponse};
use omniference::types::{ContentPart, Message as OmniMessage, Role};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_HYDRATED_IMAGES: usize = 4;

/// Hydrate a stored image into content parts. `generated` distinguishes an assistant-
/// generated image (gets an editable `image_id` handle) from a user-uploaded one.
async fn hydrate_image(db: &PgPool, image_id: &str, vision: bool, generated: bool) -> Vec<ContentPart> {
	let Ok(uuid) = Uuid::parse_str(image_id) else {
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
			_ => format!(
				"[An image was generated earlier (image_id: {image_id}) and is already displayed to the user. To modify it, call imagegen_edit with image_id \"{image_id}\".]"
			),
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
		name: Some(tool_message_name(name, call_id)),
	}
}

pub(super) fn tool_message_name(tool_name: &str, call_id: &str) -> String {
	format!("{tool_name}:{call_id}")
}

fn recent_image_ids(messages: &[Message]) -> HashSet<String> {
	let mut hydrated_image_ids = HashSet::with_capacity(MAX_HYDRATED_IMAGES);
	for message in messages.iter().rev() {
		let Some(parts) = message
			.content_parts
			.as_ref()
			.and_then(|value| serde_json::from_value::<Vec<MessagePart>>(value.clone()).ok())
		else {
			continue;
		};
		for part in parts.into_iter().rev() {
			if let MessagePart::Image { image_id } = part {
				hydrated_image_ids.insert(image_id);
				if hydrated_image_ids.len() == MAX_HYDRATED_IMAGES {
					break;
				}
			}
		}
		if hydrated_image_ids.len() == MAX_HYDRATED_IMAGES {
			break;
		}
	}
	hydrated_image_ids
}

async fn user_parts(db: &PgPool, stored_parts: Vec<MessagePart>, content: String, vision: bool, hydrated_image_ids: &HashSet<String>) -> Vec<ContentPart> {
	let mut parts = Vec::new();
	for part in stored_parts {
		match part {
			MessagePart::Text { text } => parts.push(ContentPart::Text(text)),
			MessagePart::Image { image_id } => parts.extend(hydrate_image(db, &image_id, vision && hydrated_image_ids.contains(&image_id), false).await),
			_ => {}
		}
	}
	if parts.is_empty() {
		parts.push(ContentPart::Text(content));
	}
	parts
}

async fn assistant_messages(
	db: &PgPool,
	stored_parts: Vec<MessagePart>,
	execs: Option<&Vec<ToolExecutionResponse>>,
	vision: bool,
	hydrated_image_ids: &HashSet<String>,
) -> Vec<OmniMessage> {
	let mut result = Vec::new();
	let mut call_names: HashMap<String, String> = HashMap::new();
	let mut unresolved_calls: Vec<String> = Vec::new();
	let mut buffer: Vec<ContentPart> = Vec::new();

	for part in stored_parts {
		match part {
			MessagePart::Text { text } => buffer.push(ContentPart::Text(text)),
			MessagePart::Reasoning { .. } => {}
			MessagePart::Image { image_id } => buffer.extend(hydrate_image(db, &image_id, vision && hydrated_image_ids.contains(&image_id), true).await),
			MessagePart::ToolCall { id, name, arguments } => {
				call_names.insert(id.clone(), name.clone());
				unresolved_calls.push(id.clone());
				buffer.push(ContentPart::ToolCall { id, name, arguments });
			}
			MessagePart::ToolResult { tool_call_id } => {
				if !buffer.is_empty() {
					result.push(OmniMessage {
						role: Role::Assistant,
						parts: std::mem::take(&mut buffer),
						name: None,
					});
				}
				unresolved_calls.retain(|c| c != &tool_call_id);
				result.push(tool_message(execs, &call_names, &tool_call_id));
			}
		}
	}

	if !buffer.is_empty() {
		result.push(OmniMessage {
			role: Role::Assistant,
			parts: buffer,
			name: None,
		});
	}
	for call_id in unresolved_calls {
		result.push(tool_message(execs, &call_names, &call_id));
	}
	result
}

pub(super) async fn get_omni_messages(db: &PgPool, messages: Vec<Message>, vision: bool) -> Vec<OmniMessage> {
	let hydrated_image_ids = if vision { recent_image_ids(&messages) } else { HashSet::new() };
	let assistant_ids: Vec<Uuid> = messages.iter().filter(|m| m.role == "assistant").map(|m| m.id).collect();
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

		if is_assistant {
			result.extend(assistant_messages(db, stored_parts, executions.get(&message_id), vision, &hydrated_image_ids).await);
		} else {
			let parts = user_parts(db, stored_parts, m.content, vision, &hydrated_image_ids).await;
			result.push(OmniMessage {
				role: Role::User,
				parts,
				name: None,
			});
		}
	}
	result
}
