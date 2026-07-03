//! Models routes.
//!
//! Public endpoint for listing available AI models.

use crate::config::Config;
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::catalog::{AvailabilityState, GatewayCatalogModel};
use crate::types::models::{Model, ModelListParams, ModelViewer};
use crate::types::models_configs::ModelConfig;
use crate::utils::providers::sync_endpoint_options;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::Json;
use axum::extract::{Path, Query};
use axum::{extract::State, response::IntoResponse};
use serde::Deserialize;
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SetFavoriteRequest {
	pub is_favorite: bool,
}

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

/// GET /api/v1/models/:id/provider-options
///
/// Upstream provider options for a runnable model, used by the chat provider selector. Gated on
/// the instance-wide `enable_provider_selector` setting. Endpoints are fetched lazily on first
/// open (when nothing is stored yet and the parent catalog model is available), mirroring the
/// admin endpoint but available to any authenticated user.
pub async fn get_provider_options(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let Some(user) = get_current_user(&state.db, &cookies).await else {
		return ErrorBuilder::new(ErrorCode::NotAuthenticated).build();
	};

	// Feature-gated: when disabled, behave as if there are no provider options.
	if !Config::get().enable_provider_selector() {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	match Model::can_user_use_model(&state.db, &user.id, &id).await {
		Ok(true) => {}
		Ok(false) => return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build(),
		Err(e) => {
			eprintln!("[PUBLIC] Failed to check provider option access: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	let mut options = match GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
		Ok(options) => options,
		Err(e) => {
			eprintln!("[PUBLIC] Failed to load provider options: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let is_available = matches!(options.availability_state, Some(AvailabilityState::Available));
	if options.options.is_empty() && is_available {
		if let Err(e) = sync_endpoint_options(&state.db, &id).await {
			eprintln!("[PUBLIC] Lazy endpoint fetch failed for {id}: {e}");
		} else if let Ok(refreshed) = GatewayCatalogModel::provider_options_for_model(&state.db, &id).await {
			options = refreshed;
		}
	}

	ResponseBuilder::new(ResponseBody::Json(options)).build()
}

/// POST /api/v1/models/:id/favorite
///
/// Toggle or set the favorite status of a model for the current user.
pub async fn set_model_favorite(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(body): Json<SetFavoriteRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	match Model::can_user_use_model(&state.db, &user.id, &id).await {
		Ok(true) => {}
		Ok(false) => return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build(),
		Err(e) => {
			eprintln!("[PUBLIC] Failed to check favorite model access: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Err(e) = ModelConfig::set_user_favorite(&state.db, &user.id, &id, body.is_favorite).await {
		eprintln!("[PUBLIC] Failed to set model favorite: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

	ResponseBuilder::<()>::new(ResponseBody::Empty).build()
}
