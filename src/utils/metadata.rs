//! Provider Metadata Service
//!
//! Handles resolution of provider icons, display names, and branding
//! with support for built-in defaults and database overrides.

use crate::types::{ProviderKind, ProviderMetadata};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Cached provider metadata for fast resolution
static METADATA_CACHE: std::sync::OnceLock<Arc<RwLock<MetadataCache>>> = std::sync::OnceLock::new();

#[derive(Default)]
struct MetadataCache {
	/// Metadata by provider_kind (for exact kind match)
	by_kind: HashMap<String, ProviderMetadata>,
	/// Metadata by name pattern (for openai_compat variants like "groq", "together")
	by_pattern: Vec<(String, ProviderMetadata)>,
}

/// Initialize the metadata cache from the database
pub async fn init(pool: &PgPool) {
	let cache = Arc::new(RwLock::new(MetadataCache::default()));

	// Load all metadata entries ordered by priority
	let entries = sqlx::query_as::<_, ProviderMetadata>("SELECT * FROM provider_metadata ORDER BY priority DESC")
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let mut cache_write = cache.write().await;
	for entry in entries {
		if entry.name_pattern.is_some() {
			// Pattern-based match (for openai_compat variants)
			cache_write.by_pattern.push((entry.name_pattern.clone().unwrap_or_default().to_lowercase(), entry));
		} else if let Some(ref kind) = entry.provider_kind {
			// Kind-based match
			let kind_str = kind_to_string(kind);
			cache_write.by_kind.insert(kind_str, entry);
		}
	}
	drop(cache_write);

	let _ = METADATA_CACHE.set(cache);
	println!("[AI] Provider metadata cache initialized");
}

/// Resolve provider metadata for display
pub async fn resolve(kind: &ProviderKind, provider_name: &str) -> ResolvedMetadata {
	let cache = match METADATA_CACHE.get() {
		Some(c) => c.read().await,
		None => return ResolvedMetadata::default_for_kind(kind),
	};

	let name_lower = provider_name.to_lowercase();

	// First, try pattern match (for openai_compat variants)
	for (pattern, metadata) in &cache.by_pattern {
		if name_lower.contains(pattern) {
			return ResolvedMetadata::from_metadata(metadata);
		}
	}

	// Then, try exact kind match
	let kind_str = kind_to_string(kind);
	if let Some(metadata) = cache.by_kind.get(&kind_str) {
		return ResolvedMetadata::from_metadata(metadata);
	}

	// Fallback to defaults
	ResolvedMetadata::default_for_kind(kind)
}

/// Reload metadata cache from database
pub async fn reload(pool: &PgPool) {
	let cache = match METADATA_CACHE.get() {
		Some(c) => c,
		None => return,
	};

	let entries = sqlx::query_as::<_, ProviderMetadata>("SELECT * FROM provider_metadata ORDER BY priority DESC")
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let mut cache_write = cache.write().await;
	cache_write.by_kind.clear();
	cache_write.by_pattern.clear();

	for entry in entries {
		if entry.name_pattern.is_some() {
			cache_write.by_pattern.push((entry.name_pattern.clone().unwrap_or_default().to_lowercase(), entry));
		} else if let Some(ref kind) = entry.provider_kind {
			let kind_str = kind_to_string(kind);
			cache_write.by_kind.insert(kind_str, entry);
		}
	}
}

/// Resolved metadata for a provider
#[derive(Debug, Clone)]
pub struct ResolvedMetadata {
	pub display_name: String,
	pub icon_svg: Option<String>,
	pub icon_url: Option<String>,
	pub brand_color: Option<String>,
	pub website_url: Option<String>,
}

impl ResolvedMetadata {
	fn from_metadata(m: &ProviderMetadata) -> Self {
		Self {
			display_name: m.display_name.clone(),
			icon_svg: m.icon_svg.clone(),
			icon_url: m.icon_url.clone(),
			brand_color: m.brand_color.clone(),
			website_url: m.website_url.clone(),
		}
	}

	fn default_for_kind(kind: &ProviderKind) -> Self {
		let (name, color) = match kind {
			ProviderKind::Openai => ("OpenAI", "#10A37F"),
			ProviderKind::OpenaiCompat => ("OpenAI Compatible", "#6366F1"),
			ProviderKind::Openrouter => ("OpenRouter", "#6366F1"),
			ProviderKind::Anthropic => ("Anthropic", "#D97757"),
			ProviderKind::Google => ("Google AI", "#4285F4"),
			ProviderKind::Ollama => ("Ollama", "#FFFFFF"),
			ProviderKind::Homl => ("HoML", "#FFFFFF"),
			ProviderKind::Lmstudio => ("LM Studio", "#1E1E1E"),
			ProviderKind::Custom => ("Custom", "#888888"),
		};

		Self {
			display_name: name.to_string(),
			icon_svg: None,
			icon_url: None,
			brand_color: Some(color.to_string()),
			website_url: None,
		}
	}
}

fn kind_to_string(kind: &ProviderKind) -> String {
	match kind {
		ProviderKind::Openai => "openai",
		ProviderKind::OpenaiCompat => "openai_compat",
		ProviderKind::Openrouter => "openrouter",
		ProviderKind::Anthropic => "anthropic",
		ProviderKind::Google => "google",
		ProviderKind::Ollama => "ollama",
		ProviderKind::Homl => "homl",
		ProviderKind::Lmstudio => "lmstudio",
		ProviderKind::Custom => "custom",
	}
	.to_string()
}
