//! Models routes.
//!
//! Public endpoint for listing available AI models.

use crate::config::Config;
use crate::types::catalog::{AvailabilityState, GatewayCatalogModel};
use crate::types::models::{Model, ModelListParams, ModelViewer};
use crate::types::models_configs::ModelConfig;
use crate::types::{AppError, AuthUser, Budget, BudgetSubject, JobState, PageQuery, json_ok};
use crate::utils::providers::sync_endpoint_options;
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SetFavoriteRequest {
	pub is_favorite: bool,
}

/// GET /api/v1/models
///
/// List all available AI models from the database.
pub async fn list_models(State(state): State<Arc<JobState>>, user: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ModelListParams>) -> Result<Response, AppError> {
	let viewer = ModelViewer { user_id: &user.id };
	let mut models = Model::list_for_user(
		&state.db,
		viewer,
		&page,
		false,
		params.query.as_deref(),
		params.is_favorite.unwrap_or(false),
		params.provider_id.as_ref(),
	)
	.await?;

	let blocked_ids: HashSet<Uuid> = match Budget::status_for(&state.db, BudgetSubject::user(user.id)).await {
		Ok(status) => status.blocked_model_ids.into_iter().collect(),
		Err(e) => {
			tracing::warn!(error = %e, "failed to load budget status for model list");
			HashSet::new()
		}
	};
	for model in &mut models.items {
		model.budget_blocked = blocked_ids.contains(&model.id);
	}

	json_ok(models)
}

/// GET /api/v1/models/:id/provider-options
///
/// Upstream provider options for a runnable model, used by the chat provider selector. Gated on
/// the instance-wide `enable_provider_selector` setting. Endpoints are fetched lazily on first
/// open (when nothing is stored yet and the parent catalog model is available), mirroring the
/// admin endpoint but available to any authenticated user.
#[allow(clippy::cognitive_complexity)]
pub async fn get_provider_options(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	if !Config::get().enable_provider_selector() {
		return Err(ErrorCode::InsufficientPermissions.into());
	}

	if !Model::can_user_use_model(&state.db, &user.id, &id).await? {
		return Err(ErrorCode::InsufficientPermissions.into());
	}

	let mut options = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await?;

	let is_available = matches!(options.availability_state, Some(AvailabilityState::Available));
	if options.options.is_empty() && is_available {
		if let Err(e) = sync_endpoint_options(&state.db, &id).await {
			tracing::warn!(model_id = %id, error = %e, "lazy endpoint fetch failed");
		} else if let Ok(refreshed) = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
			options = refreshed;
		}
	}

	json_ok(options)
}

/// POST /api/v1/models/:id/favorite
///
/// Toggle or set the favorite status of a model for the current user.
pub async fn set_model_favorite(State(state): State<Arc<JobState>>, user: AuthUser, Path(id): Path<Uuid>, Json(body): Json<SetFavoriteRequest>) -> Result<Response, AppError> {
	if !Model::can_user_use_model(&state.db, &user.id, &id).await? {
		return Err(ErrorCode::InsufficientPermissions.into());
	}

	ModelConfig::set_user_favorite(&state.db, &user.id, &id, body.is_favorite).await?;

	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).build())
}
