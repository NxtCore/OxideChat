//! Image CDN routes for serving and uploading images.
//!
//! Both endpoints require an authenticated session:
//! - GET /api/v1/images/:id - Serve an image by UUID
//! - POST /api/v1/images - Upload a base64 image

use crate::types::{AppError, AuthUser, JobState, UploadImageRequest, UploadImageResponse, json_ok};
use crate::utils::images::{get_image, image_url, safe_image_mime, store_from_data_uri};
use crate::utils::response::ErrorCode;
use axum::{
	Json,
	extract::{Path, State},
	http::{HeaderMap, StatusCode, header},
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// Upload a base64 image and return its URL
///
/// POST /api/v1/images
pub async fn upload_image(State(state): State<Arc<JobState>>, user: AuthUser, Json(req): Json<UploadImageRequest>) -> Result<Response, AppError> {
	let stored = store_from_data_uri(&state.db, &req.data_uri, Some(user.id), req.source.as_deref()).await.map_err(|e| {
		tracing::warn!(error = %e, "failed to store uploaded image");
		ErrorCode::BadRequest
	})?;

	json_ok(UploadImageResponse {
		id: stored.id,
		url: image_url(stored.id),
		mime_type: stored.mime_type,
		size_bytes: stored.size_bytes,
	})
}

/// Serve an image by ID
///
/// GET /api/v1/images/:id
pub async fn serve_image(State(state): State<Arc<JobState>>, _user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let (data, mime_type) = get_image(&state.db, id)
		.await
		.map_err(|e| {
			tracing::error!(image_id = %id, error = %e, "failed to retrieve image");
			ErrorCode::InternalError
		})?
		.ok_or(ErrorCode::NotFound)?;

	let mut headers = HeaderMap::new();
	headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("private, max-age=3600"));
	headers.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));

	if let Some(safe_mime) = safe_image_mime(&data, &mime_type) {
		headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static(safe_mime));
	} else {
		headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
		headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment"));
	}

	Ok((StatusCode::OK, headers, data).into_response())
}
