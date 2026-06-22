//! Providers routes.
//!
//! Public endpoint for listing providers that have at least one enabled model.

use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::models::{Model, ModelViewer};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{extract::State, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

/// GET /api/v1/providers
///
/// List distinct providers that have at least one enabled model.
pub async fn list_providers(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
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
