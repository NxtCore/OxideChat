//! Models routes.
//!
//! Public endpoint for listing available AI models.

use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::models::{Model, ModelListParams, ModelViewer};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::Query;
use axum::{extract::State, response::IntoResponse};
use std::sync::Arc;
use tower_cookies::Cookies;

/// GET /api/v1/models
///
/// List all available AI models from the database.
pub async fn list_models(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ModelListParams>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	let viewer = ModelViewer { user_id: &user.id };
	let models = match Model::list_for_user(
		&state.db,
		viewer,
		params.page.unwrap_or(1),
		params.size.unwrap_or(0),
		false,
		params.query.as_deref(),
		params.is_favorite.unwrap_or(false),
		params.provider_id.as_ref(),
	)
	.await
	{
		Ok(m) => m,
		Err(e) => {
			eprintln!("[PUBLIC] Failed to list models: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(models)).build()
}

