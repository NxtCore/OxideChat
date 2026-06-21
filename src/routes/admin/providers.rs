//! Admin AI Provider management routes.
//!
//! CRUD operations for system-wide AI providers.

use crate::ai::parse_extra_headers;
use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::models::Model;
use crate::types::providers::{
	CreateProviderRequest, Provider, ProviderResponse, SyncProviderResponse, TestProviderRequest, TestProviderResponse, UpdateProviderRequest,
};

use crate::utils::encryption::{decrypt_api_key, encrypt_api_key};
use crate::utils::providers::sync_provider_models;
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{Path, State},
	http::StatusCode,
	response::IntoResponse,
};
use omniference::{
	OmniferenceEngine,
	types::{ProviderConfig, ProviderEndpoint},
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

pub const ADMIN_PROVIDERS_VIEW: &str = "admin.providers.view";
pub const ADMIN_PROVIDERS_EDIT: &str = "admin.providers.edit";

/// List all system providers
pub async fn list_providers(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let providers = Provider::list_for_admin(&state.db).await;

	match providers {
		Ok(providers) => {
			let responses: Vec<ProviderResponse> = providers.into_iter().map(Into::into).collect();
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
		Err(e) => {
			eprintln!("[AI] Failed to list providers: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Get a single provider by ID
pub async fn get_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let provider = Provider::find_for_admin(&state.db, &id).await;

	match provider {
		Ok(Some(provider)) => {
			let response: ProviderResponse = provider.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[AI] Failed to get provider: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Create a new system provider
pub async fn create_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<CreateProviderRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let api_key = req.api_key.as_ref().map(|k| encrypt_api_key(k));
	let base_url = req.base_url.trim_end_matches('/').to_string();

	let provider = Provider::create_system(&state.db, &req.kind, &req.name, &base_url, api_key.as_deref(), &req.extra_headers, req.is_enabled).await;

	match provider {
		Ok(provider) => {
			if let Err(e) = sync_provider_models(&state.db, &provider).await {
				eprintln!("[AI] Warning: Failed to sync models for new provider: {e}");
			}
			let response: ProviderResponse = provider.into();
			ResponseBuilder::new(ResponseBody::Json(response)).status(StatusCode::CREATED).build()
		}
		Err(e) => {
			if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
				ErrorBuilder::new(ErrorCode::AlreadyExists).build()
			} else {
				eprintln!("[AI] Failed to create provider: {e}");
				ErrorBuilder::new(ErrorCode::InternalError).build()
			}
		}
	}
}

/// Update an existing provider
pub async fn update_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<UpdateProviderRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let base_url = req.base_url.as_deref().map(|url| url.trim_end_matches('/').to_string());
	let api_key = req.api_key.as_ref().map(|key| encrypt_api_key(key));
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
	.await;

	match provider {
		Ok(Some(provider)) => {
			if let Err(e) = sync_provider_models(&state.db, &provider).await {
				eprintln!("[AI] Warning: Failed to sync models after provider update: {e}");
			}
			let response: ProviderResponse = provider.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[AI] Failed to update provider: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Delete a provider
pub async fn delete_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let result = Provider::delete_system(&state.db, &id).await;

	match result {
		Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(false) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[AI] Failed to delete provider: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Test a provider connection (can use either an existing provider ID or inline config)
pub async fn test_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let provider = Provider::find_for_admin(&state.db, &id).await;

	let provider = match provider {
		Ok(Some(p)) => p,
		Ok(None) => {
			return ErrorBuilder::new(ErrorCode::NotFound).build();
		}
		Err(e) => {
			eprintln!("[AI] Failed to fetch provider for test: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers.0);

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

	// Test the connection
	match test_provider_connection(config).await {
		Ok(result) => ResponseBuilder::new(ResponseBody::Json(result)).build(),
		Err(e) => {
			let result = TestProviderResponse {
				success: false,
				models_found: 0,
				message: e,
			};
			ResponseBuilder::new(ResponseBody::Json(result)).build()
		}
	}
}

/// Test a provider with inline configuration (doesn't require existing provider)
pub async fn test_provider_inline(State(state): State<Arc<JobState>>, cookies: Cookies, Json(req): Json<TestProviderRequest>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let extra_headers = parse_extra_headers(&req.extra_headers);

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

	match test_provider_connection(config).await {
		Ok(result) => ResponseBuilder::new(ResponseBody::Json(result)).build(),
		Err(e) => {
			let result = TestProviderResponse {
				success: false,
				models_found: 0,
				message: e,
			};
			ResponseBuilder::new(ResponseBody::Json(result)).build()
		}
	}
}

/// Sync models from a provider
pub async fn sync_provider(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let provider = Provider::find_for_admin(&state.db, &id).await;

	let provider = match provider {
		Ok(Some(p)) => p,
		Ok(None) => {
			return ErrorBuilder::new(ErrorCode::NotFound).build();
		}
		Err(e) => {
			eprintln!("[AI] Failed to fetch provider for sync: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	match sync_provider_models(&state.db, &provider).await {
		Ok(response) => ResponseBuilder::new(ResponseBody::Json(response)).build(),
		Err(e) => ResponseBuilder::new(ResponseBody::Json(SyncProviderResponse {
			success: false,
			models_added: 0,
			models_updated: 0,
			models_removed: 0,
			message: e,
		}))
		.build(),
	}
}

/// List models for a provider
pub async fn list_models(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_PROVIDERS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let models = Model::list_by_provider_for_admin(&state.db, &id).await;

	match models {
		Ok(models) => ResponseBuilder::new(ResponseBody::Json(models)).build(),
		Err(e) => {
			eprintln!("[AI] Failed to list models: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
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
