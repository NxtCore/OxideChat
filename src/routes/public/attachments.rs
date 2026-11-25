//! Files attached to chat messages.
//!
//! - POST /api/v1/chat-attachments?filename=… - Upload a file (the raw body) and extract its text

use crate::types::{AppError, AuthUser, ChatAttachment, JobState, UploadAttachmentQuery, json_ok};
use axum::{
	body::Bytes,
	extract::{Query, State},
	response::Response,
};
use std::sync::Arc;

/// Upload a file whose text the next message carries.
///
/// POST /api/v1/chat-attachments
pub async fn upload_attachment(
	State(state): State<Arc<JobState>>,
	user: AuthUser,
	Query(query): Query<UploadAttachmentQuery>,
	body: Bytes,
) -> Result<Response, AppError> {
	let attachment = ChatAttachment::upload(&state.db, &user.id, &query.filename, &body).await?;
	json_ok(attachment.to_response())
}
