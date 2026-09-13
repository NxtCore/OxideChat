//! Providers routes.
//!
//! Public endpoint for listing providers that have at least one enabled model.

use crate::types::{JobState, RequestContext};
use crate::types::models::{Model, ModelViewer};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{extract::{Extension, State}, response::IntoResponse};
use std::sync::Arc;

/// GET /api/v1/providers
///
/// List distinct providers that have at least one enabled model.
pub async fn list_providers(State(state): State<Arc<JobState>>, Extension(RequestContext { user: current_user }): Extension<RequestContext>) -> impl IntoResponse {
	let user = match current_user {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let viewer = ModelViewer { user_id: &user.id };
	let providers = match Model::list_providers_for_user(&state.db, viewer).await {
		Ok(p) => p,
		Err(e) => {
			eprintln!("[PUBLIC] Failed to list model providers: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(providers)).build()
}
