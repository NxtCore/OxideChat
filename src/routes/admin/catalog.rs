//! Admin gateway-catalog routes: the OpenRouter catalog/search view and a runnable model's
//! provider-options table (endpoints fetched lazily).

use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::catalog::GatewayCatalogModel;
use crate::types::models::ModelListParams;
use crate::utils::providers::sync_endpoint_options;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::Query;
use axum::{
	extract::{Path, State},
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

const ADMIN_PROVIDERS_VIEW: &str = "admin.providers.view";
const ADMIN_PROVIDERS_EDIT: &str = "admin.providers.edit";
const OPENROUTER_GATEWAY: &str = "openrouter";

/// GET /api/v1/admin/providers/:id/catalog
///
/// Paginated catalog/search listing for a provider, including `USER_UNAVAILABLE` models.
pub async fn list_catalog(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Query(params): Query<ModelListParams>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match GatewayCatalogModel::list_for_admin(&state.db, &id, OPENROUTER_GATEWAY, params.page.unwrap_or(1), params.size.unwrap_or(0), params.query).await {
		Ok(catalog) => ResponseBuilder::new(ResponseBody::Json(catalog)).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to list catalog: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// GET /api/v1/admin/models/:id/provider-options
///
/// Provider options for a runnable model. Endpoints are fetched lazily on first open (when no
/// options are stored yet and the parent catalog model is available).
pub async fn list_provider_options(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let mut options = match GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
		Ok(options) => options,
		Err(e) => {
			eprintln!("[ADMIN] Failed to load provider options: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Lazy first fetch: only when nothing is stored yet and the model is runnable.
	let is_available = matches!(options.availability_state, Some(crate::types::catalog::AvailabilityState::Available));
	if options.options.is_empty() && is_available {
		if let Err(e) = sync_endpoint_options(&state.db, &id).await {
			eprintln!("[ADMIN] Lazy endpoint fetch failed for {id}: {e}");
		} else if let Ok(refreshed) = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
			options = refreshed;
		}
	}

	ResponseBuilder::new(ResponseBody::Json(options)).build()
}

/// POST /api/v1/admin/models/:id/provider-options/refresh
///
/// Force a re-fetch of provider endpoints (skips `USER_UNAVAILABLE` models in the syncer).
pub async fn refresh_provider_options(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	if let Err(e) = sync_endpoint_options(&state.db, &id).await {
		eprintln!("[ADMIN] Endpoint refresh failed for {id}: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

	match GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
		Ok(options) => ResponseBuilder::new(ResponseBody::Json(options)).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to load provider options after refresh: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
