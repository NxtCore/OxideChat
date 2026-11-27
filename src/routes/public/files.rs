//! Files users store: images and chat attachments.
//!
//! - `POST /api/v1/files?filename=…&purpose=image|icon|chat_attachment` - Upload a file (the raw body)
//! - `GET /api/v1/files/{id}` - Serve an image's bytes; the instance icon is served without a session

use crate::config::Config;
use crate::types::{AppError, AuthUser, FileError, FileUpload, JobState, RequestContext, UploadFileQuery, json_ok};
use crate::utils::images::{get_image, get_image_for_user, safe_image_mime};
use crate::utils::response::ErrorCode;
use axum::{
	body::Bytes,
	extract::{Extension, Path, Query, State, rejection::BytesRejection},
	http::{HeaderMap, StatusCode, header},
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// Upload a file and store it the way its purpose asks for.
///
/// POST /api/v1/files
pub async fn upload_file(State(state): State<Arc<JobState>>, user: AuthUser, Query(query): Query<UploadFileQuery>, body: Result<Bytes, BytesRejection>) -> Result<Response, AppError> {
	let body = match body {
		Ok(body) => body,
		Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => return Err(FileError::TooLarge.into()),
		Err(rejection) => return Ok(rejection.into_response()),
	};
	json_ok(FileUpload::store(&state.db, &user.id, &query, &body).await?)
}

/// Serve an image's bytes by ID.
///
/// GET /api/v1/files/:id
pub async fn serve_file(State(state): State<Arc<JobState>>, Extension(RequestContext { user }): Extension<RequestContext>, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let is_instance_icon = Config::get().instance_icon_id() == Some(id);
	let found = match (is_instance_icon, user) {
		(true, _) => get_image(&state.db, id).await,
		(false, Some(user)) => get_image_for_user(&state.db, id, user.id).await,
		(false, None) => return Err(ErrorCode::NotAuthenticated.into()),
	};

	let (data, mime_type) = found
		.map_err(|e| {
			tracing::error!(file_id = %id, error = %e, "failed to retrieve file");
			ErrorCode::InternalError
		})?
		.ok_or(ErrorCode::NotFound)?;

	let mut headers = HeaderMap::new();
	let cache_control = if is_instance_icon { "public, max-age=86400" } else { "private, max-age=3600" };
	headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static(cache_control));
	headers.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));

	if let Some(safe_mime) = safe_image_mime(&data, &mime_type) {
		headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static(safe_mime));
	} else {
		headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
		headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment"));
	}

	Ok((StatusCode::OK, headers, data).into_response())
}
