//! Admin gateway-catalog routes: the `OpenRouter` catalog/search view and a runnable model's
//! provider-options table (endpoints fetched lazily).

use crate::types::catalog::GatewayCatalogModel;
use crate::types::consts::{ADMIN_PROVIDERS_EDIT, ADMIN_PROVIDERS_VIEW};
use crate::types::models::ModelListParams;
use crate::types::{AppError, AuthUser, JobState, PageQuery, json_ok};
use crate::utils::providers::sync_endpoint_options;
use crate::utils::response::ErrorCode;
use axum::{
	extract::{Path, Query, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

const OPENROUTER_GATEWAY: &str = "openrouter";

/// GET /api/v1/admin/providers/:id/catalog
///
/// Paginated catalog/search listing for a provider, including `USER_UNAVAILABLE` models.
pub async fn list_catalog(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Query(page): Query<PageQuery>, Query(params): Query<ModelListParams>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let catalog = GatewayCatalogModel::list_for_admin(&state.db, &id, OPENROUTER_GATEWAY, &page, params.query).await?;
	json_ok(catalog)
}

/// GET /api/v1/admin/models/:id/provider-options
///
/// Provider options for a runnable model. Endpoints are fetched lazily on first open (when no
/// options are stored yet and the parent catalog model is available).
#[allow(clippy::cognitive_complexity)]
pub async fn list_provider_options(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let mut options = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await?;

	// Lazy first fetch: only when nothing is stored yet and the model is runnable.
	let is_available = matches!(options.availability_state, Some(crate::types::catalog::AvailabilityState::Available));
	if options.options.is_empty() && is_available {
		if let Err(e) = sync_endpoint_options(&state.db, &id).await {
			tracing::error!("Lazy endpoint fetch failed for {id}: {e}");
		} else if let Ok(refreshed) = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
			options = refreshed;
		}
	}

	json_ok(options)
}

/// POST /api/v1/admin/models/:id/provider-options/refresh
///
/// Force a re-fetch of provider endpoints (skips `USER_UNAVAILABLE` models in the syncer).
pub async fn refresh_provider_options(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	sync_endpoint_options(&state.db, &id)
		.await
		.inspect_err(|e| tracing::error!("Endpoint refresh failed for {id}: {e}"))
		.map_err(|_| ErrorCode::InternalError)?;

	let options = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await?;
	json_ok(options)
}
