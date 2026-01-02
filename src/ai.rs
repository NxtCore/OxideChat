//! AI Engine module for OxideChat.
//!
//! Manages the shared OmniferenceEngine instance and provides utilities
//! for AI provider operations through the SDK.
//!
//! Note: The embedded HTTP API (`/ai/*`) requires omniference to be on the same
//! axum version. Currently, we use the SDK approach only for internal operations.

use omniference::{
	OmniferenceEngine,
	types::{ProviderConfig, ProviderEndpoint},
};
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

use crate::types::AiProvider;
use crate::utils::encryption::decrypt_api_key;

/// Someone kill me for this name please
pub static OF_ENGINE: std::sync::OnceLock<Arc<RwLock<OmniferenceEngine>>> = std::sync::OnceLock::new();

/// Initialize the AI engine with providers from the database
pub async fn init(pool: &PgPool) {
	let engine = Arc::new(RwLock::new(OmniferenceEngine::new()));

	let providers = sqlx::query_as::<_, AiProvider>("SELECT * FROM providers WHERE owner_id IS NULL AND is_enabled = true")
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let provider_count = providers.len();
	let mut engine_write = engine.write().await;
	for provider in providers {
		let config = provider_to_config(&provider);
		if let Err(e) = engine_write.register_provider(config).await {
			eprintln!("[AI] Failed to register provider '{}': {}", provider.name, e);
		} else {
			println!("[AI] Registered provider '{}'", provider.name);
		}
	}
	drop(engine_write);

	let _ = OF_ENGINE.set(engine);
	println!("[AI] Engine initialized with {} system providers", provider_count);
}

/// Get the global AI engine
#[must_use]
pub fn get() -> Arc<RwLock<OmniferenceEngine>> {
	OF_ENGINE.get().expect("AI engine not initialized").clone()
}

/// Reload providers from the database
pub async fn reload_providers(pool: &PgPool) {
	let providers = sqlx::query_as::<_, AiProvider>("SELECT * FROM providers WHERE owner_id IS NULL AND is_enabled = true")
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let provider_count = providers.len();

	// Create a fresh engine with new providers
	let new_engine = OmniferenceEngine::new();
	let engine_arc = get();
	let mut engine_write = engine_arc.write().await;
	*engine_write = new_engine;

	for provider in providers {
		let config = provider_to_config(&provider);
		if let Err(e) = engine_write.register_provider(config).await {
			eprintln!("[AI] Failed to register provider '{}': {}", provider.name, e);
		}
	}
	println!("[AI] Reloaded {} providers", provider_count);
}

/// Convert a database provider to omniference config
pub fn provider_to_config(provider: &AiProvider) -> ProviderConfig {
	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers);

	ProviderConfig {
		name: provider.name.clone(),
		endpoint: ProviderEndpoint {
			kind: provider.kind.to_omni_kind(),
			base_url: provider.base_url.clone(),
			api_key,
			extra_headers,
			timeout: None,
		},
		enabled: provider.is_enabled,
	}
}

fn parse_extra_headers(value: &serde_json::Value) -> BTreeMap<String, String> {
	if let Some(obj) = value.as_object() {
		obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
	} else {
		BTreeMap::new()
	}
}
