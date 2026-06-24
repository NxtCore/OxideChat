//! Optimized provider utility functions

use crate::ai::OF_ENGINE;
use crate::ai::parse_extra_headers;
use crate::types::catalog::{AvailabilityState, GatewayCatalogModel, GatewayCatalogSyncInput, GatewayProviderOptionSyncInput};
use crate::types::models::{Model, ModelSyncInput};
use crate::types::providers::{Provider, ProviderKind, SyncProviderResponse};
use crate::utils::encryption::decrypt_api_key;
use omniference::types::{ProviderConfig, ProviderEndpoint};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Source-gateway tag stored on catalog rows for OpenRouter providers.
const OPENROUTER_GATEWAY: &str = "openrouter";

pub async fn sync_provider_models(pool: &PgPool, provider: &Provider) -> Result<SyncProviderResponse, String> {
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

	let engine_arc = OF_ENGINE.get().ok_or_else(|| "AI engine not initialized".to_string())?;
	let engine = engine_arc.read().await;
	let engine_service = engine.service();
	if engine_service.provider_manager().read().await.get_provider(&config.name).is_none() {
		if let Err(e) = engine_service.register_provider(config.clone()).await {
			return Err(format!("Connection failed: {e}"));
		}
	}

	let all_discovered = match engine_service.discover_models().await {
		Ok(models) => models,
		Err(e) => return Err(format!("Discovery failed: {e}")),
	};

	let discovered: Vec<_> = all_discovered.into_iter().filter(|m| m.provider_name == provider.name).collect();

	let mut sync_models = Vec::with_capacity(discovered.len());
	for model in discovered {
		let capabilities_json = serde_json::to_value(&model.capabilities).map_err(|e| format!("Failed to serialize capabilities for model {}: {}", model.id, e))?;
		let input_modalities_json =
			serde_json::to_value(&model.input_modalities).map_err(|e| format!("Failed to serialize input_modalities for model {}: {}", model.id, e))?;
		let output_modalities_json =
			serde_json::to_value(&model.output_modalities).map_err(|e| format!("Failed to serialize output_modalities for model {}: {}", model.id, e))?;

		sync_models.push(ModelSyncInput {
			provider_id: provider.id,
			model_id: model.id,
			display_name: model.name,
			capabilities: capabilities_json,
			input_modalities: input_modalities_json,
			output_modalities: output_modalities_json,
			context_length: model.context_length.map(|c| c as i32),
			max_tokens: model.max_tokens.map(|c| c as i32),
		});
	}

	let summary = Model::sync_provider_models(pool, &provider.id, &sync_models)
		.await
		.map_err(|e| format!("Failed to sync provider models: {e}"))?;

	// Piggyback the gateway catalog sync on the existing provider sync (OpenRouter only).
	// Failures here are non-fatal: runnable model sync already succeeded.
	if provider.kind == ProviderKind::Openrouter {
		if let Err(e) = sync_gateway_catalog(pool, provider).await {
			eprintln!("[AI] Warning: gateway catalog sync failed for {}: {e}", provider.name);
		}
	}

	Ok(SyncProviderResponse {
		success: true,
		models_added: summary.added,
		models_updated: summary.updated,
		models_removed: summary.removed,
		message: format!("Synced {} models", sync_models.len()),
	})
}

/// Sync the OpenRouter gateway catalog: upsert the public catalog, flag models the configured
/// key cannot run as `USER_UNAVAILABLE`, prune rows absent from the public catalog, and relink
/// `local_model_id` to runnable `models`.
pub async fn sync_gateway_catalog(pool: &PgPool, provider: &Provider) -> Result<(), String> {
	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers.0);
	let base_url = provider.base_url.as_str();

	let public = omniference::catalog::fetch_public_models(base_url, api_key.as_deref(), &extra_headers, None)
		.await
		.map_err(|e| format!("Public catalog fetch failed: {e}"))?;
	let user = omniference::catalog::fetch_user_models(base_url, api_key.as_deref(), &extra_headers, None)
		.await
		.map_err(|e| format!("User catalog fetch failed: {e}"))?;

	// Never wipe the catalog on an empty/failed public response.
	if public.is_empty() {
		return Ok(());
	}

	let inputs: Vec<GatewayCatalogSyncInput> = public
		.iter()
		.map(|model| GatewayCatalogSyncInput {
			gateway_model_id: model.id.clone(),
			display_name: if model.name.is_empty() { model.id.clone() } else { model.name.clone() },
			raw: serde_json::to_value(model).unwrap_or(Value::Null),
		})
		.collect();
	let public_ids: Vec<String> = public.iter().map(|model| model.id.clone()).collect();
	let user_ids: Vec<String> = user.iter().map(|model| model.id.clone()).collect();

	GatewayCatalogModel::upsert_public_catalog(pool, &provider.id, OPENROUTER_GATEWAY, &inputs)
		.await
		.map_err(|e| format!("Catalog upsert failed: {e}"))?;
	GatewayCatalogModel::mark_availability(pool, &provider.id, OPENROUTER_GATEWAY, &user_ids)
		.await
		.map_err(|e| format!("Catalog availability update failed: {e}"))?;
	GatewayCatalogModel::delete_absent(pool, &provider.id, OPENROUTER_GATEWAY, &public_ids)
		.await
		.map_err(|e| format!("Catalog prune failed: {e}"))?;
	GatewayCatalogModel::refresh_local_model_ids(pool, &provider.id, OPENROUTER_GATEWAY, &provider.name)
		.await
		.map_err(|e| format!("Catalog relink failed: {e}"))?;

	Ok(())
}

/// Lazily fetch and store provider endpoint options for a runnable model. Skips models whose
/// parent catalog row is `USER_UNAVAILABLE` (no point fetching endpoints for a disabled model)
/// and models with no linked catalog row.
pub async fn sync_endpoint_options(pool: &PgPool, model_id: &Uuid) -> Result<(), String> {
	let Some(target) = GatewayCatalogModel::find_catalog_for_model(pool, model_id)
		.await
		.map_err(|e| format!("Failed to resolve catalog model: {e}"))?
	else {
		return Ok(());
	};

	if target.availability_state == AvailabilityState::UserUnavailable {
		return Ok(());
	}

	let provider = Provider::find_for_admin(pool, &target.provider_id)
		.await
		.map_err(|e| format!("Failed to load provider: {e}"))?
		.ok_or_else(|| "Provider not found".to_string())?;

	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers.0);

	let (author, slug) = target
		.gateway_model_id
		.split_once('/')
		.ok_or_else(|| format!("Invalid gateway model id: {}", target.gateway_model_id))?;

	let endpoints = omniference::catalog::fetch_model_endpoints(&provider.base_url, author, slug, api_key.as_deref(), &extra_headers, None)
		.await
		.map_err(|e| format!("Endpoint fetch failed: {e}"))?;

	let items: Vec<GatewayProviderOptionSyncInput> = endpoints
		.endpoints
		.iter()
		.map(|endpoint| {
			let pricing = endpoint.pricing.as_ref().and_then(omniference::catalog::pricing_from_catalog);
			GatewayProviderOptionSyncInput {
				provider_slug: endpoint.tag.clone(),
				provider_name: endpoint.provider_name.clone(),
				endpoint_name: endpoint.name.clone(),
				status: endpoint.status,
				quantization: endpoint.quantization.clone(),
				context_length: endpoint.context_length.map(|c| c as i32),
				max_completion_tokens: endpoint.max_completion_tokens.map(|c| c as i32),
				latency: endpoint.latency_last_30m,
				throughput: endpoint.throughput_last_30m,
				uptime: endpoint.uptime_last_30m,
				price_input: pricing.as_ref().map(|p| p.input),
				price_output: pricing.as_ref().map(|p| p.output),
				raw: serde_json::to_value(endpoint).unwrap_or(Value::Null),
			}
		})
		.collect();

	GatewayCatalogModel::upsert_provider_options(pool, &target.catalog_id, &items)
		.await
		.map_err(|e| format!("Failed to store provider options: {e}"))?;

	Ok(())
}
