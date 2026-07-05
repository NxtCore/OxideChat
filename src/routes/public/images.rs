//! Image CDN routes for serving and uploading images.
//!
//! Provides public endpoints for image storage:
//! - GET /api/v1/images/:id - Serve an image by UUID
//! - POST /api/v1/images - Upload a base64 image (internal use)

use crate::types::JobState;
use crate::types::{UploadImageRequest, UploadImageResponse};
use crate::utils::images::{get_image, image_url, safe_image_mime, store_from_data_uri};
use axum::{
	Json,
	extract::{Path, State},
	http::{HeaderMap, StatusCode, header},
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

use super::auth::get_current_user;

/// Upload a base64 image and return its URL
///
/// POST /api/images
pub async fn upload_image(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Json(req): Json<UploadImageRequest>,
) -> Result<Json<UploadImageResponse>, (StatusCode, String)> {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return Err((StatusCode::UNAUTHORIZED, "Not authenticated".to_string()));
	};

	let stored = store_from_data_uri(&state.db, &req.data_uri, Some(user.id), req.source.as_deref())
		.await
		.map_err(|e| (StatusCode::BAD_REQUEST, e))?;

	Ok(Json(UploadImageResponse {
		id: stored.id,
		url: image_url(stored.id),
		mime_type: stored.mime_type,
		size_bytes: stored.size_bytes,
	}))
}

/// Serve an image by ID
///
/// GET /api/v1/images/:id
pub async fn serve_image(State(state): State<Arc<JobState>>, Path(id): Path<Uuid>) -> Response {
	match get_image(&state.db, id).await {
		Ok(Some((data, mime_type))) => {
			let mut headers = HeaderMap::new();
			headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("public, max-age=31536000, immutable"));
			headers.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));

			if let Some(safe_mime) = safe_image_mime(&data, &mime_type) {
				headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static(safe_mime));
			} else {
				headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
				headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment"));
			}

			(StatusCode::OK, headers, data).into_response()
		}
		Ok(None) => (StatusCode::NOT_FOUND, "Image not found").into_response(),
		Err(e) => {
			eprintln!("[IMAGES] Failed to retrieve image {id}: {e}");
			(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
		}
	}
}
