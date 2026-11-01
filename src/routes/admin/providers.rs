//! Admin AI Provider management routes.
//!
//! CRUD operations for system-wide AI providers.

use crate::ai::{parse_extra_headers, reload_providers};
use crate::types::consts::{ADMIN_PROVIDERS_EDIT, ADMIN_PROVIDERS_VIEW};
use crate::types::{AppError, AuthUser, JobState};
use crate::types::models::Model;
use crate::types::providers::{
	CreateProviderRequest, Provider, ProviderResponse, SyncProviderResponse, TestProviderRequest, TestProviderResponse, UpdateProviderRequest,
};

use crate::utils::encryption::{decrypt_api_key, encrypt_api_key};
use crate::utils::providers::sync_provider_models;
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use omniference::{
	OmniferenceEngine,
	types::{ProviderConfig, ProviderEndpoint},
};
use std::sync::Arc;
use uuid::Uuid;

/// List all system providers
pub async fn list_providers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let responses: Vec<ProviderResponse> = Provider::list_for_admin(&state.db).await?.into_iter().map(Into::into).collect();

	Ok(ResponseBuilder::new(ResponseBody::Json(responses)).build())
}

/// Get a single provider by ID
pub async fn get_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let provider = Provider::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	let response: ProviderResponse = provider.into();

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// Create a new system provider
pub async fn create_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<CreateProviderRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let api_key = req.api_key.as_deref().map(encrypt_api_key).transpose().map_err(|error| {
		tracing::error!("Failed to encrypt provider credential: {error}");
		ErrorCode::InternalError
	})?;
	let base_url = req.base_url.trim_end_matches('/').to_string();

	let provider = Provider::create_system(&state.db, &req.kind, &req.name, &base_url, api_key.as_deref(), &req.extra_headers, req.is_enabled)
		.await
		.map_err(|e| AppError::unique_or(e, ErrorCode::AlreadyExists))?;

	reload_providers(&state.db).await;
	if provider.is_enabled
		&& let Err(e) = sync_provider_models(&state.db, &provider).await
	{
		tracing::warn!("Failed to sync models for new provider: {e}");
	}
	let response: ProviderResponse = provider.into();

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build())
}

/// Update an existing provider
pub async fn update_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateProviderRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let base_url = req.base_url.as_deref().map(|url| url.trim_end_matches('/').to_string());
	let api_key = req.api_key.as_deref().map(encrypt_api_key).transpose().map_err(|error| {
		tracing::error!("Failed to encrypt provider credential: {error}");
		ErrorCode::InternalError
	})?;
	let provider = Provider::patch_system(
		&state.db,
		&id,
		req.kind.as_ref(),
		req.name.as_deref(),
		base_url.as_deref(),
		api_key.as_deref().map(Some),
		req.extra_headers.as_ref(),
		req.is_enabled,
	)
	.await?
	.ok_or(ErrorCode::NotFound)?;

	reload_providers(&state.db).await;
	if provider.is_enabled
		&& let Err(e) = sync_provider_models(&state.db, &provider).await
	{
		tracing::warn!("Failed to sync models after provider update: {e}");
	}
	let response: ProviderResponse = provider.into();

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// Delete a provider
pub async fn delete_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	if !Provider::delete_system(&state.db, &id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	reload_providers(&state.db).await;

	Ok((StatusCode::NO_CONTENT, "").into_response())
}

/// Test a provider connection (can use either an existing provider ID or inline config)
pub async fn test_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let provider = Provider::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;

	let api_key = provider.api_key.as_deref().map(decrypt_api_key).transpose().map_err(|error| {
		tracing::error!(provider_id=%provider.id, "Failed to decrypt provider credential: {error}");
		ErrorCode::InternalError
	})?;
	let extra_headers = parse_extra_headers(&provider.extra_headers.0).map_err(|error| {
		tracing::error!(provider_id=%provider.id, "Failed to decrypt provider headers: {error}");
		ErrorCode::InternalError
	})?;

	let config = ProviderConfig {
		name: provider.name.clone(),
		endpoint: ProviderEndpoint {
			kind: provider.kind.to_omni_kind(),
			base_url: provider.base_url.clone(),
			api_key,
			extra_headers,
			timeout: None,
		},
		enabled: true,
		catalog_provider_slug: None,
	};

	let result = test_provider_connection(config).await.unwrap_or_else(|message| TestProviderResponse {
		success: false,
		models_found: 0,
		message,
	});

	Ok(ResponseBuilder::new(ResponseBody::Json(result)).build())
}

/// Test a provider with inline configuration (doesn't require existing provider)
pub async fn test_provider_inline(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<TestProviderRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let extra_headers = parse_extra_headers(&req.extra_headers).map_err(|error| {
		tracing::error!("Failed to read provider headers: {error}");
		ErrorCode::InternalError
	})?;

	let config = ProviderConfig {
		name: "test".to_string(),
		endpoint: ProviderEndpoint {
			kind: req.kind.to_omni_kind(),
			base_url: req.base_url,
			api_key: req.api_key,
			extra_headers,
			timeout: None,
		},
		enabled: true,
		catalog_provider_slug: None,
	};

	let result = test_provider_connection(config).await.unwrap_or_else(|message| TestProviderResponse {
		success: false,
		models_found: 0,
		message,
	});

	Ok(ResponseBuilder::new(ResponseBody::Json(result)).build())
}

/// Sync models from a provider
pub async fn sync_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let provider = Provider::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;

	let response = sync_provider_models(&state.db, &provider).await.unwrap_or_else(|message| SyncProviderResponse {
		success: false,
		models_added: 0,
		models_updated: 0,
		models_removed: 0,
		message,
	});

	Ok(ResponseBuilder::new(ResponseBody::Json(response)).build())
}

/// List models for a provider
pub async fn list_models(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let models = Model::list_by_provider_for_admin(&state.db, &id).await?;

	Ok(ResponseBuilder::new(ResponseBody::Json(models)).build())
}

async fn test_provider_connection(config: ProviderConfig) -> Result<TestProviderResponse, String> {
	let mut engine = OmniferenceEngine::new();

	engine.register_provider(config).await.map_err(|e| format!("Connection failed: {e}"))?;

	let models = engine.discover_models().await.map_err(|e| format!("Discovery failed: {e}"))?;

	Ok(TestProviderResponse {
		success: true,
		models_found: models.len(),
		message: format!("Successfully connected, found {} models", models.len()),
	})
}
