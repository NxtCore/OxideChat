//! AI Engine module for `OxideChat`.
//!
//! Manages the shared `OmniferenceEngine` instance and provides utilities
//! for AI provider operations through the SDK.
//!
//! Note: The embedded HTTP API (`/ai/*`) requires omniference to be on the same
//! axum version. Currently, we use the SDK approach only for internal operations.

use omniference::{
	OmniferenceEngine,
	catalog::Catalog,
	types::{ProviderConfig, ProviderEndpoint},
};
use arc_swap::ArcSwap;
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};

use crate::types::{JobState, ProviderRoutingMiddleware};
use crate::types::models::ModelPricing;
use crate::types::providers::{Provider, ProviderKind};
use crate::utils::encryption::decrypt_api_key;
use crate::utils::omniference_cost::OxideCostQueue;

/// Someone kill me for this name please
pub static OF_ENGINE: std::sync::OnceLock<ArcSwap<OmniferenceEngine>> = std::sync::OnceLock::new();
static OF_COST_QUEUE: std::sync::OnceLock<Arc<OxideCostQueue>> = std::sync::OnceLock::new();
static OF_PROVIDER_ROUTING: std::sync::OnceLock<Arc<ProviderRoutingMiddleware>> = std::sync::OnceLock::new();
static BUILT_IN_CATALOG: std::sync::OnceLock<Arc<Catalog>> = std::sync::OnceLock::new();

/// Initialize the AI engine with providers from the database
pub async fn init(state: &Arc<JobState>) {
	let pool = &state.db;
	let cost_sink = Arc::clone(OF_COST_QUEUE.get_or_init(|| OxideCostQueue::spawn(Arc::clone(state))));
	let routing = Arc::clone(OF_PROVIDER_ROUTING.get_or_init(|| Arc::new(ProviderRoutingMiddleware::new(Arc::clone(state)))));
	let (engine, provider_count) = build_engine(pool, cost_sink, routing).await;

	let _ = OF_ENGINE.set(ArcSwap::from_pointee(engine));
	println!("[AI] Engine initialized with {} system providers", provider_count);
}

/// The Omniference reported-cost config for a provider; warns when its currency has no rate yet.
fn reported_cost_config(provider: &Provider, usd_per_unit: &std::collections::HashMap<String, rust_decimal::Decimal>) -> Option<omniference::reported_cost::ReportedCost> {
	let config = provider.reported_cost.as_ref()?.0.to_omniference(usd_per_unit);
	if config.is_none() {
		tracing::warn!(provider_id=%provider.id, "no exchange rate for the provider's reported cost currency; using token prices");
	}
	config
}

/// Builds an engine with every enabled system provider registered and returns it with the provider count.
async fn build_engine(pool: &PgPool, cost_sink: Arc<OxideCostQueue>, routing: Arc<ProviderRoutingMiddleware>) -> (OmniferenceEngine, usize) {
	let mut engine = OmniferenceEngine::with_cost_sink(cost_sink);
	engine.service().add_middleware(routing);
	let providers = Provider::list_enabled_system(pool).await.unwrap_or_default();
	let usd_per_unit = crate::types::exchange_rates::ExchangeRate::usd_per_unit_map(pool).await.unwrap_or_default();

	for provider in &providers {
		let config = match provider_to_config(provider) {
			Ok(mut config) => {
				config.endpoint.reported_cost = reported_cost_config(provider, &usd_per_unit);
				config
			}
			Err(error) => {
				tracing::error!(provider_id=%provider.id, "Failed to decrypt provider credential: {error}");
				continue;
			}
		};

		if let Err(e) = engine.register_provider(config).await {
			eprintln!("[AI] Failed to register provider '{}': {}", provider.name, e);
		} else {
			println!("[AI] Registered provider '{}'", provider.name);
		}
	}

	load_pricing_overrides(pool, &engine).await;
	(engine, providers.len())
}

/// Loads every manual model price into the engine's catalog, so Omniference prices requests with it.
async fn load_pricing_overrides(pool: &PgPool, engine: &OmniferenceEngine) {
	let catalog = engine.service().catalog();
	match ModelPricing::engine_overrides(pool, &catalog).await {
		Ok(overrides) => catalog.replace_pricing_overrides(overrides).await,
		Err(error) => tracing::error!("Failed to load manual model prices: {error}"),
	}
}

/// The running engine's model catalog, or the built-in models.dev snapshot before the engine starts.
#[must_use]
pub fn catalog() -> Arc<Catalog> {
	match OF_ENGINE.get() {
		Some(engine) => engine.load().service().catalog(),
		None => Arc::clone(BUILT_IN_CATALOG.get_or_init(|| Arc::new(Catalog::from_env().unwrap_or_default()))),
	}
}

/// A provider config with only what catalog lookups read: the provider's name and kind.
#[must_use]
pub fn catalog_provider(name: &str, kind: &ProviderKind) -> ProviderConfig {
	ProviderConfig {
		name: name.to_string(),
		endpoint: ProviderEndpoint {
			kind: kind.to_omni_kind(),
			base_url: String::new(),
			api_key: None,
			extra_headers: BTreeMap::new(),
			timeout: None,
			reported_cost: None,
		},
		enabled: true,
		catalog_provider_slug: kind.catalog_slug(),
	}
}

/// Get the global AI engine
#[must_use]
pub fn get() -> Arc<OmniferenceEngine> {
	OF_ENGINE.get().expect("AI engine not initialized").load_full()
}

/// Drains pending inference usage before application state is closed.
pub async fn shutdown() {
	if let Some(cost_queue) = OF_COST_QUEUE.get() {
		cost_queue.shutdown().await;
	}
}

/// Rebuilds the engine from the providers in the database and swaps it in.
///
/// Call this after a provider is created, updated or deleted so the next request uses the
/// stored configuration. Discovery runs before the swap, so in-flight requests are not blocked.
pub async fn reload_providers(pool: &PgPool) {
	let (Some(cost_sink), Some(routing), Some(engine_arc)) = (OF_COST_QUEUE.get().map(Arc::clone), OF_PROVIDER_ROUTING.get().map(Arc::clone), OF_ENGINE.get()) else {
		tracing::error!("cannot reload providers before the AI engine is initialized");
		return;
	};
	let (new_engine, provider_count) = build_engine(pool, cost_sink, routing).await;
	engine_arc.store(Arc::new(new_engine));
	load_pricing_overrides(pool, &engine_arc.load()).await;
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
			reported_cost: None,
		},
		enabled: provider.is_enabled,
		catalog_provider_slug: provider.kind.catalog_slug(),
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
