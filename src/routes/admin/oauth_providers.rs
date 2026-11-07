//! Admin routes for sign-in providers.

use crate::logging::{AuditLog, LogEvent};
use crate::types::consts::{ADMIN_CONFIG_EDIT, ADMIN_CONFIG_VIEW};
use crate::types::oauth::{OAuthProvider, ResolvedEndpoints, SaveOAuthProviderRequest};
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::oauth;
use crate::utils::response::ErrorCode;
use axum::{
	Json,
	extract::{Path, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

async fn resolve(req: &SaveOAuthProviderRequest) -> Result<ResolvedEndpoints, ErrorCode> {
	oauth::resolve_endpoints(&req.endpoints).await.map_err(|e| {
		tracing::warn!("[OAUTH] Rejected provider endpoints: {e}");
		ErrorCode::BadRequest
	})
}

/// GET /api/v1/admin/oauth-providers
pub async fn list_oauth_providers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_VIEW).await?;
	let providers = OAuthProvider::list(&state.db).await?;
	json_ok(providers.iter().map(OAuthProvider::to_response).collect::<Vec<_>>())
}

/// GET /api/v1/admin/oauth-providers/presets
pub async fn list_oauth_presets(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_VIEW).await?;
	json_ok(OAuthProvider::presets())
}

/// POST /api/v1/admin/oauth-providers
pub async fn create_oauth_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<SaveOAuthProviderRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;
	if !req.is_valid(true) {
		return Err(ErrorCode::BadRequest.into());
	}
	let endpoints = resolve(&req).await?;
	let provider = OAuthProvider::create(&state.db, &req, &endpoints)
		.await
		.map_err(|e| AppError::unique_or(e, ErrorCode::AlreadyExists))?;
	AuditLog::log(&state.db, LogEvent::OAuthProviderCreated, Some(auth.id), None, Some(provider.id));
	json_ok(provider.to_response())
}

/// PUT /api/v1/admin/oauth-providers/{id}
///
/// An empty client secret keeps the stored one.
pub async fn update_oauth_provider(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Json(req): Json<SaveOAuthProviderRequest>,
) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;
	if !req.is_valid(false) {
		return Err(ErrorCode::BadRequest.into());
	}
	let endpoints = resolve(&req).await?;
	let provider = OAuthProvider::update(&state.db, id, &req, &endpoints).await?.ok_or(ErrorCode::NotFound)?;
	AuditLog::log(&state.db, LogEvent::OAuthProviderUpdated, Some(auth.id), None, Some(provider.id));
	json_ok(provider.to_response())
}

/// DELETE /api/v1/admin/oauth-providers/{id}
pub async fn delete_oauth_provider(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;
	if !OAuthProvider::delete(&state.db, id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	AuditLog::log(&state.db, LogEvent::OAuthProviderDeleted, Some(auth.id), None, Some(id));
	json_ok(serde_json::json!({}))
}
