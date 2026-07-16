use super::providers::{ADMIN_PROVIDERS_EDIT, ADMIN_PROVIDERS_VIEW};
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::providers::{Provider, ProviderKind, UpdateProviderBillingRequest};
use crate::utils::encryption::{encrypt_api_key, is_enabled};
use crate::utils::provider_billing::refresh_provider_billing;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

pub async fn list_billing(State(state): State<Arc<JobState>>, cookies: Cookies) -> Response {
	if let Err(response) = authorize(&state, &cookies, ADMIN_PROVIDERS_VIEW).await {
		return response;
	}
	match Provider::list_billing_overviews_for_admin(&state.db).await {
		Ok(overviews) => ResponseBuilder::new(ResponseBody::Json(overviews)).build(),
		Err(error) => {
			tracing::error!("Failed to list provider billing overviews: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn get_billing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> Response {
	if let Err(response) = authorize(&state, &cookies, ADMIN_PROVIDERS_VIEW).await {
		return response;
	}
	overview_response(&state, &id).await
}

pub async fn update_billing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(request): Json<UpdateProviderBillingRequest>) -> Response {
	if let Err(response) = authorize(&state, &cookies, ADMIN_PROVIDERS_EDIT).await {
		return response;
	}
	let provider = match Provider::find_for_admin(&state.db, &id).await {
		Ok(Some(provider)) => provider,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(_) => return ErrorBuilder::new(ErrorCode::InternalError).build(),
	};
	if request.credential.as_ref().is_some_and(|value| value.trim().is_empty()) {
		return ErrorBuilder::new(ErrorCode::ValidationFailed).field("credential").build();
	}
	if request.credential.is_some() && !is_enabled() {
		return ErrorBuilder::new(ErrorCode::BadRequest)
			.details(serde_json::json!({"reason": "encryption_required"}))
			.build();
	}
	if matches!(
		provider.kind,
		ProviderKind::Google | ProviderKind::OpenaiCompat | ProviderKind::Custom | ProviderKind::Anthropic
	) && request.is_enabled
	{
		return ErrorBuilder::new(ErrorCode::BadRequest).details(serde_json::json!({"reason": "unsupported"})).build();
	}
	let existing = Provider::find_billing_connection_for_admin(&state.db, &id).await.ok().flatten();
	if provider.kind == ProviderKind::Openai && request.is_enabled {
		let has_key = request.credential.is_some() || existing.as_ref().is_some_and(|connection| connection.credential.is_some());
		let has_project = request.external_scope_id.as_deref().is_some_and(|value| !value.trim().is_empty());
		if !has_key || !has_project {
			return ErrorBuilder::new(ErrorCode::ValidationFailed)
				.details(serde_json::json!({"reason": "openai_billing_configuration_required"}))
				.build();
		}
	}
	let encrypted = request.credential.as_deref().map(encrypt_api_key);
	match Provider::upsert_billing_connection_for_admin(&state.db, &id, &request, encrypted.as_deref()).await {
		Ok(Some(_)) => overview_response(&state, &id).await,
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(error) => {
			tracing::error!(provider_id=%id, "Failed to save provider billing configuration: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn delete_billing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> Response {
	if let Err(response) = authorize(&state, &cookies, ADMIN_PROVIDERS_EDIT).await {
		return response;
	}
	match Provider::delete_billing_connection_for_admin(&state.db, &id).await {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(error) => {
			tracing::error!(provider_id=%id, "Failed to remove provider billing configuration: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn refresh_billing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> Response {
	if let Err(response) = authorize(&state, &cookies, ADMIN_PROVIDERS_EDIT).await {
		return response;
	}
	let provider = match Provider::find_for_admin(&state.db, &id).await {
		Ok(Some(provider)) => provider,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(_) => return ErrorBuilder::new(ErrorCode::InternalError).build(),
	};
	if let Err(error) = refresh_provider_billing(&state.db, &provider).await {
		tracing::warn!(provider_id=%provider.id, provider_name=%provider.name, category=error.code(), "Provider billing refresh failed");
	}
	overview_response(&state, &id).await
}

async fn authorize(state: &JobState, cookies: &Cookies, permission: &str) -> Result<(), Response> {
	let Some(user) = get_current_user(&state.db, cookies).await else {
		return Err(ErrorBuilder::new(ErrorCode::NotAuthenticated).build());
	};
	if !user.has_permission(&state.db, permission).await {
		return Err(ErrorBuilder::new(ErrorCode::InsufficientPermissions).build());
	}
	Ok(())
}

async fn overview_response(state: &JobState, id: &Uuid) -> Response {
	match Provider::billing_overview_for_admin(&state.db, id).await {
		Ok(Some(overview)) => ResponseBuilder::new(ResponseBody::Json(overview)).build(),
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(error) => {
			tracing::error!(provider_id=%id, "Failed to load provider billing overview: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}
