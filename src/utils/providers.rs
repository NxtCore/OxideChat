//! Provider utility functions

use crate::ai::OF_ENGINE;
use crate::types::{AiModel, AiProvider, ModelCapabilities, SyncProviderResponse};
use crate::utils::encryption::decrypt_api_key;
use omniference::{
	OmniferenceEngine,
	types::{ProviderConfig, ProviderEndpoint},
};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashSet};

pub async fn sync_provider_models(pool: &PgPool, provider: &AiProvider) -> Result<SyncProviderResponse, String> {
	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers);

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
	};

	let engine = OF_ENGINE.get().expect("AI engine not initialized").read().await;
	let engine_service = engine.service();
	if engine.service().provider_manager().read().await.get_provider(&config.name).is_none() {
		if let Err(e) = engine_service.register_provider(config.clone()).await {
			return Err(format!("Connection failed: {e}"));
		}
	}

	let discovered = match engine_service.discover_models().await {
		Ok(models) => models,
		Err(e) => return Err(format!("Discovery failed: {e}")),
	};

	let existing = sqlx::query_as::<_, AiModel>("SELECT * FROM ai_models WHERE provider_id = $1")
		.bind(provider.id)
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let existing_ids: HashSet<_> = existing.iter().map(|m| m.model_id.as_str()).collect();
	let discovered_ids: HashSet<_> = discovered.iter().map(|m| m.id.as_str()).collect();

	let mut added = 0;
	let mut updated = 0;
	let mut removed = 0;

	for model in &discovered {
		let capabilities: ModelCapabilities = model.capabilities.clone().into();
		let capabilities_json = serde_json::to_value(&capabilities).unwrap_or_default();
		let modalities_json = serde_json::to_value(&model.modalities).unwrap_or_default();

		if existing_ids.contains(model.id.as_str()) {
			let _ = sqlx::query(
				r#"
				UPDATE ai_models SET 
					display_name = $2, capabilities = $3, modalities = $4, 
					context_length = $5, max_tokens = $6, updated_at = NOW()
				WHERE provider_id = $1 AND model_id = $7
				"#,
			)
			.bind(provider.id)
			.bind(&model.name)
			.bind(&capabilities_json)
			.bind(&modalities_json)
			.bind(model.capabilities.context_length.map(|c| c as i32))
			.bind(model.capabilities.max_tokens.map(|c| c as i32))
			.bind(&model.id)
			.execute(pool)
			.await;
			updated += 1;
		} else {
			let _ = sqlx::query(
				r#"
				INSERT INTO ai_models (provider_id, model_id, display_name, capabilities, modalities, context_length, max_tokens)
				VALUES ($1, $2, $3, $4, $5, $6, $7)
				"#,
			)
			.bind(provider.id)
			.bind(&model.id)
			.bind(&model.name)
			.bind(&capabilities_json)
			.bind(&modalities_json)
			.bind(model.capabilities.context_length.map(|c| c as i32))
			.bind(model.capabilities.max_tokens.map(|c| c as i32))
			.execute(pool)
			.await;
			added += 1;
		}
	}

	for model in &existing {
		if !discovered_ids.contains(model.model_id.as_str()) {
			let _ = sqlx::query("DELETE FROM ai_models WHERE id = $1").bind(model.id).execute(pool).await;
			removed += 1;
		}
	}

	Ok(SyncProviderResponse {
		success: true,
		models_added: added,
		models_updated: updated,
		models_removed: removed,
		message: format!("Synced {} models", discovered.len()),
	})
}

fn parse_extra_headers(value: &serde_json::Value) -> BTreeMap<String, String> {
	if let Some(obj) = value.as_object() {
		obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
	} else {
		BTreeMap::new()
	}
}
