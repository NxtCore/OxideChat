use crate::types::consts::{ADMIN_PROVIDERS_EDIT, ADMIN_PROVIDERS_VIEW};
use crate::types::providers::{Provider, ProviderKind, UpdateProviderBillingRequest};
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::encryption::{encrypt_api_key, is_enabled};
use crate::utils::provider_billing::refresh_provider_billing;
use crate::utils::response::{ErrorBuilder, ErrorCode};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_billing(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let overviews = Provider::list_billing_overviews_for_admin(&state.db).await?;
	json_ok(overviews)
}

pub async fn get_billing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	overview_response(&state, &id).await
}

pub async fn update_billing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(request): Json<UpdateProviderBillingRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let provider = Provider::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	if request.credential.as_ref().is_some_and(|value| value.trim().is_empty()) {
		return Ok(ErrorBuilder::new(ErrorCode::ValidationFailed).field("credential").build());
	}
	if request.credential.is_some() && !is_enabled() {
		return Ok(ErrorBuilder::new(ErrorCode::BadRequest)
			.details(serde_json::json!({"reason": "encryption_required"}))
			.build());
	}
	if matches!(
		provider.kind,
		ProviderKind::Openai | ProviderKind::Google | ProviderKind::OpenaiCompat | ProviderKind::Custom | ProviderKind::Anthropic
	) {
		return Ok(ErrorBuilder::new(ErrorCode::BadRequest).details(serde_json::json!({"reason": "unsupported"})).build());
	}
	let encrypted = request
		.credential
		.as_deref()
		.map(encrypt_api_key)
		.transpose()
		.inspect_err(|error| tracing::error!(provider_id=%id, "Failed to encrypt billing credential: {error}"))
		.map_err(|_| ErrorCode::InternalError)?;
	Provider::upsert_billing_connection_for_admin(&state.db, &id, encrypted.as_deref()).await?.ok_or(ErrorCode::NotFound)?;
	overview_response(&state, &id).await
}

pub async fn delete_billing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	if !Provider::delete_billing_connection_for_admin(&state.db, &id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	Ok((StatusCode::NO_CONTENT, "").into_response())
}

pub async fn refresh_billing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let provider = Provider::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	if let Err(error) = refresh_provider_billing(&state.db, &provider).await {
		tracing::warn!(provider_id=%provider.id, provider_name=%provider.name, category=error.code(), "Provider billing refresh failed");
	}
	overview_response(&state, &id).await
}

async fn overview_response(state: &JobState, id: &Uuid) -> Result<Response, AppError> {
	let overview = Provider::billing_overview_for_admin(&state.db, id).await?.ok_or(ErrorCode::NotFound)?;
	json_ok(overview)
}
