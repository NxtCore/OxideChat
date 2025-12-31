//! Admin AI Provider management routes.
//!
//! CRUD operations for system-wide AI providers.

use crate::AppState;
use crate::types::{
	AiModel, AiProvider, CreateProviderRequest, ModelResponse, ProviderResponse, SyncProviderResponse, TestProviderRequest, TestProviderResponse, UpdateProviderRequest,
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
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

/// List all system providers
pub async fn list_providers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
	let providers = sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers WHERE owner_id IS NULL ORDER BY name")
		.fetch_all(&state.db)
		.await;

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
pub async fn get_provider(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let provider = sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers WHERE id = $1 AND owner_id IS NULL")
		.bind(id)
		.fetch_optional(&state.db)
		.await;

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
pub async fn create_provider(State(state): State<Arc<AppState>>, Json(req): Json<CreateProviderRequest>) -> impl IntoResponse {
	let api_key = req.api_key.as_ref().map(|k| encrypt_api_key(k));
	let base_url = req.base_url.trim_end_matches('/').to_string();

	let provider = sqlx::query_as::<_, AiProvider>(
		r#"
		INSERT INTO ai_providers (owner_id, kind, name, base_url, api_key, extra_headers, is_enabled)
		VALUES (NULL, $1, $2, $3, $4, $5, $6)
		RETURNING *
		"#,
	)
	.bind(&req.kind)
	.bind(&req.name)
	.bind(&base_url)
	.bind(&api_key)
	.bind(&req.extra_headers)
	.bind(req.is_enabled)
	.fetch_one(&state.db)
	.await;

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
pub async fn update_provider(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<UpdateProviderRequest>) -> impl IntoResponse {
	// First get the existing provider
	let existing = sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers WHERE id = $1 AND owner_id IS NULL")
		.bind(id)
		.fetch_optional(&state.db)
		.await;

	let existing = match existing {
		Ok(Some(p)) => p,
		Ok(None) => {
			return ErrorBuilder::new(ErrorCode::NotFound).build();
		}
		Err(e) => {
			eprintln!("[AI] Failed to fetch provider for update: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	// Apply updates
	let kind = req.kind.unwrap_or(existing.kind);
	let name = req.name.unwrap_or(existing.name);
	let base_url = req.base_url.unwrap_or(existing.base_url);
	let api_key = match req.api_key {
		Some(key) => Some(encrypt_api_key(&key)),
		None => existing.api_key,
	};
	let extra_headers = req.extra_headers.unwrap_or(existing.extra_headers);
	let is_enabled = req.is_enabled.unwrap_or(existing.is_enabled);

	let provider = sqlx::query_as::<_, AiProvider>(
		r#"
		UPDATE ai_providers 
		SET kind = $2, name = $3, base_url = $4, api_key = $5, extra_headers = $6, is_enabled = $7, updated_at = NOW()
		WHERE id = $1 AND owner_id IS NULL
		RETURNING *
		"#,
	)
	.bind(id)
	.bind(&kind)
	.bind(&name)
	.bind(&base_url)
	.bind(&api_key)
	.bind(&extra_headers)
	.bind(is_enabled)
	.fetch_one(&state.db)
	.await;

	match provider {
		Ok(provider) => {
			if let Err(e) = sync_provider_models(&state.db, &provider).await {
				eprintln!("[AI] Warning: Failed to sync models after provider update: {e}");
			}
			let response: ProviderResponse = provider.into();
			ResponseBuilder::new(ResponseBody::Json(response)).build()
		}
		Err(e) => {
			eprintln!("[AI] Failed to update provider: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Delete a provider
pub async fn delete_provider(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let result = sqlx::query("DELETE FROM ai_providers WHERE id = $1 AND owner_id IS NULL")
		.bind(id)
		.execute(&state.db)
		.await;

	match result {
		Ok(res) if res.rows_affected() > 0 => (StatusCode::NO_CONTENT, "").into_response(),
		Ok(_) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[AI] Failed to delete provider: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// Test a provider connection (can use either an existing provider ID or inline config)
pub async fn test_provider(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
	// Get the provider from database
	let provider = sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers WHERE id = $1 AND owner_id IS NULL")
		.bind(id)
		.fetch_optional(&state.db)
		.await;

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

	// Build omniference config
	let api_key = provider.api_key.map(|k| decrypt_api_key(&k));
	let extra_headers = parse_extra_headers(&provider.extra_headers);

	let config = ProviderConfig {
		name: provider.name.clone(),
		endpoint: ProviderEndpoint {
			kind: provider.kind.to_omni_kind(),
			base_url: provider.base_url,
			api_key,
			extra_headers,
			timeout: None,
		},
		enabled: true,
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
pub async fn test_provider_inline(Json(req): Json<TestProviderRequest>) -> impl IntoResponse {
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
pub async fn sync_provider(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let provider = sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers WHERE id = $1 AND owner_id IS NULL")
		.bind(id)
		.fetch_optional(&state.db)
		.await;

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
pub async fn list_models(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
	let models = sqlx::query_as::<_, AiModel>("SELECT * FROM ai_models WHERE provider_id = $1 ORDER BY display_name")
		.bind(id)
		.fetch_all(&state.db)
		.await;

	match models {
		Ok(models) => {
			let responses: Vec<ModelResponse> = models.into_iter().map(Into::into).collect();
			ResponseBuilder::new(ResponseBody::Json(responses)).build()
		}
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

fn parse_extra_headers(value: &serde_json::Value) -> BTreeMap<String, String> {
	if let Some(obj) = value.as_object() {
		obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
	} else {
		BTreeMap::new()
	}
}
