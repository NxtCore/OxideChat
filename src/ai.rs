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

use crate::types::JobState;
use crate::types::providers::Provider;
use crate::utils::encryption::decrypt_api_key;
use crate::utils::omniference_cost::OxideCostQueue;

/// Someone kill me for this name please
pub static OF_ENGINE: std::sync::OnceLock<Arc<RwLock<OmniferenceEngine>>> = std::sync::OnceLock::new();
static OF_COST_QUEUE: std::sync::OnceLock<Arc<OxideCostQueue>> = std::sync::OnceLock::new();

/// Initialize the AI engine with providers from the database
pub async fn init(state: &Arc<JobState>) {
	let pool = &state.db;
	let cost_sink = Arc::clone(OF_COST_QUEUE.get_or_init(|| OxideCostQueue::spawn(Arc::clone(state))));
	let engine = Arc::new(RwLock::new(OmniferenceEngine::with_cost_sink(cost_sink)));

	let providers = Provider::list_enabled_system(pool).await.unwrap_or_default();

	let provider_count = providers.len();
	let mut engine_write = engine.write().await;
	for provider in providers {
		let config = match provider_to_config(&provider) {
			Ok(config) => config,
			Err(error) => {
				tracing::error!(provider_id=%provider.id, "Failed to decrypt provider credential: {error}");
				continue;
			}
		};

		if let Err(e) = engine_write.register_provider(config).await {
			eprintln!("[AI] Failed to register provider '{}': {}", provider.name, e);
		} else {
			println!("[AI] Registered provider '{}'", provider.name);
		}
	}

	drop(engine_write);

	let _ = OF_ENGINE.set(engine);
	sync_pricing_overrides(pool).await;
	println!("[AI] Engine initialized with {} system providers", provider_count);
}

/// Get the global AI engine
#[must_use]
pub fn get() -> Arc<RwLock<OmniferenceEngine>> {
	OF_ENGINE.get().expect("AI engine not initialized").clone()
}

/// Get the shared omniference catalog, if the engine is initialized.
pub async fn catalog() -> Option<Arc<omniference::catalog::Catalog>> {
	let engine = OF_ENGINE.get()?;
	let guard = engine.read().await;
	Some(guard.service().catalog().clone())
}

/// Pricing overrides are stored in the database and applied at usage-event
/// accounting time — no catalog integration needed.
pub async fn sync_pricing_overrides(_pool: &PgPool) {}

/// Drains pending inference usage before application state is closed.
pub async fn shutdown() {
	if let Some(cost_queue) = OF_COST_QUEUE.get() {
		cost_queue.shutdown().await;
	}
}

/// Reload providers from the database
pub async fn reload_providers(state: &Arc<JobState>) {
	let pool = &state.db;
	let providers = Provider::list_enabled_system(pool).await.unwrap_or_default();

	let provider_count = providers.len();

	// Create a fresh engine with new providers
	let Some(cost_sink) = OF_COST_QUEUE.get().map(Arc::clone) else {
		tracing::error!("cannot reload providers before the AI engine is initialized");
		return;
	};
	let new_engine = OmniferenceEngine::with_cost_sink(cost_sink);
	let engine_arc = get();
	let mut engine_write = engine_arc.write().await;
	*engine_write = new_engine;

	for provider in providers {
		let config = match provider_to_config(&provider) {
			Ok(config) => config,
			Err(error) => {
				tracing::error!(provider_id=%provider.id, "Failed to decrypt provider credential: {error}");
				continue;
			}
		};
		if let Err(e) = engine_write.register_provider(config).await {
			eprintln!("[AI] Failed to register provider '{}': {}", provider.name, e);
		}
	}
	drop(engine_write);
	sync_pricing_overrides(pool).await;
	println!("[AI] Reloaded {} providers", provider_count);
}

/// Convert a database provider to omniference config
pub fn provider_to_config(provider: &Provider) -> Result<ProviderConfig, crate::utils::encryption::EncryptionError> {
	let api_key = provider.api_key.as_deref().map(decrypt_api_key).transpose()?;
	let extra_headers = parse_extra_headers(&provider.extra_headers.0)?;

	Ok(ProviderConfig {
		name: provider.name.clone(),
		endpoint: ProviderEndpoint {
			kind: provider.kind.to_omni_kind(),
			base_url: provider.base_url.clone(),
			api_key,
			extra_headers,
			timeout: None,
		},
		enabled: provider.is_enabled,
		catalog_provider_slug: None,
	})
}

pub fn parse_extra_headers(value: &serde_json::Value) -> Result<BTreeMap<String, String>, crate::utils::encryption::EncryptionError> {
	if let Some(obj) = value.as_object() {
		obj.iter()
			.filter_map(|(key, value)| value.as_str().map(|secret| (key, secret)))
			.map(|(key, secret)| crate::utils::encryption::decrypt_api_key(secret).map(|value| (key.clone(), value)))
			.collect()
	} else {
		Ok(BTreeMap::new())
	}
}
