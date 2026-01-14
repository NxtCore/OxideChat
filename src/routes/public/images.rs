//! Image CDN routes for serving and uploading images.
//!
//! Provides public endpoints for image storage:
//! - GET /api/v1/images/:id - Serve an image by UUID
//! - POST /api/v1/images - Upload a base64 image (internal use)

use crate::AppState;
use crate::utils::images::{get_image, image_url, store_from_data_uri};
use axum::{
	Json,
	extract::{Path, State},
	http::{HeaderMap, StatusCode, header},
	response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Request body for uploading an image
#[derive(Debug, Deserialize)]
pub struct UploadImageRequest {
	/// Base64 data URI (e.g., "data:image/png;base64,...")
	pub data_uri: String,
	/// Optional user ID for attribution
	pub user_id: Option<Uuid>,
	/// Optional source identifier (e.g., "imagegen")
	pub source: Option<String>,
}

/// Response for successful image upload
#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
	pub id: Uuid,
	pub url: String,
	pub mime_type: String,
	pub size_bytes: i64,
}

/// Upload a base64 image and return its URL
///
/// POST /api/images
pub async fn upload_image(State(state): State<Arc<AppState>>, Json(req): Json<UploadImageRequest>) -> Result<Json<UploadImageResponse>, (StatusCode, String)> {
	let stored = store_from_data_uri(&state.db, &req.data_uri, req.user_id, req.source.as_deref())
		.await
		.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

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
pub async fn serve_image(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> Response {
	match get_image(&state.db, id).await {
		Ok(Some((data, mime_type))) => {
			let mut headers = HeaderMap::new();
			headers.insert(
				header::CONTENT_TYPE,
				mime_type.parse().unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
			);
			headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("public, max-age=31536000, immutable"));

			(StatusCode::OK, headers, data).into_response()
		}
		Ok(None) => (StatusCode::NOT_FOUND, "Image not found").into_response(),
		Err(e) => {
			eprintln!("[IMAGES] Failed to retrieve image {id}: {e}");
			(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
		}
	}
}
