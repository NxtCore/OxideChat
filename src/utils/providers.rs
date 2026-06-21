//! Optimized provider utility functions

use crate::ai::OF_ENGINE;
use crate::ai::parse_extra_headers;
use crate::types::models::{Model, ModelSyncInput};
use crate::types::providers::{Provider, SyncProviderResponse};
use crate::utils::encryption::decrypt_api_key;
use omniference::types::{ProviderConfig, ProviderEndpoint};
use sqlx::PgPool;

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

	Ok(SyncProviderResponse {
		success: true,
		models_added: summary.added,
		models_updated: summary.updated,
		models_removed: summary.removed,
		message: format!("Synced {} models", sync_models.len()),
	})
}
