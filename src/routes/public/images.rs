//! Image CDN routes for serving and uploading images.
//!
//! Both endpoints require an authenticated session:
//! - GET /api/v1/images/:id - Serve an image by UUID
//! - POST /api/v1/images - Upload a base64 image

use crate::types::{JobState, RequestContext, UploadImageRequest, UploadImageResponse};
use crate::utils::images::{get_image, image_url, safe_image_mime, store_from_data_uri};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Extension, Path, State},
	http::{HeaderMap, StatusCode, header},
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// Upload a base64 image and return its URL
///
/// POST /api/v1/images
pub async fn upload_image(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user: current_user }): Extension<RequestContext>,
	Json(req): Json<UploadImageRequest>,
) -> Response {
	let Some(user) = current_user else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	let stored = match store_from_data_uri(&state.db, &req.data_uri, Some(user.id), req.source.as_deref()).await {
		Ok(stored) => stored,
		Err(e) => {
			eprintln!("[IMAGES] Failed to store uploaded image: {e}");
			return ErrorBuilder::new(ErrorCode::BadRequest).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(UploadImageResponse {
		id: stored.id,
		url: image_url(stored.id),
		mime_type: stored.mime_type,
		size_bytes: stored.size_bytes,
	}))
	.build()
}

/// Serve an image by ID
///
/// GET /api/v1/images/:id
pub async fn serve_image(
	State(state): State<Arc<JobState>>,
	Extension(RequestContext { user: current_user }): Extension<RequestContext>,
	Path(id): Path<Uuid>,
) -> Response {
	if current_user.is_none() {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	}
	match get_image(&state.db, id).await {
		Ok(Some((data, mime_type))) => {
			let mut headers = HeaderMap::new();
			headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("private, max-age=3600"));
			headers.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));

			if let Some(safe_mime) = safe_image_mime(&data, &mime_type) {
				headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static(safe_mime));
			} else {
				headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
				headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment"));
			}

			(StatusCode::OK, headers, data).into_response()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[IMAGES] Failed to retrieve image {id}: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
