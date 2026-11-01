//! Providers routes.
//!
//! Public endpoint for listing providers that have at least one enabled model.

use crate::types::models::{Model, ModelViewer};
use crate::types::{AppError, AuthUser, JobState, json_ok};
use axum::{extract::State, response::Response};
use std::sync::Arc;

/// GET /api/v1/providers
///
/// List distinct providers that have at least one enabled model.
pub async fn list_providers(State(state): State<Arc<JobState>>, user: AuthUser) -> Result<Response, AppError> {
	let viewer = ModelViewer { user_id: &user.id };
	json_ok(Model::list_providers_for_user(&state.db, viewer).await?)
}
