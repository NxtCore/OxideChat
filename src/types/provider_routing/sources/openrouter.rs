use super::{SourceProvider, SyncedUpstreamProvider, UpstreamSnapshot, UpstreamSource, UpstreamSyncError};
use crate::types::UpstreamGuardrail;
use async_trait::async_trait;
use omniference::catalog::{OpenRouterApiKey, OpenRouterGuardrail, OpenRouterKeyAssignment};
use sha2::{Digest, Sha256};

const TIMEOUT_MS: u64 = 15_000;

/// `OpenRouter`: providers from `/v1/providers`, restrictions from the guardrail assigned to the inference key.
pub struct OpenRouterSource;

#[async_trait]
impl UpstreamSource for OpenRouterSource {
	async fn sync(&self, provider: &SourceProvider) -> Result<UpstreamSnapshot, UpstreamSyncError> {
		let listed = omniference::catalog::fetch_providers(&provider.base_url, provider.api_key.as_deref(), &provider.extra_headers, Some(TIMEOUT_MS))
			.await
			.map_err(|error| UpstreamSyncError::Request(error.to_string()))?;
		let providers = listed
			.into_iter()
			.map(|listed| SyncedUpstreamProvider {
				slug: listed.slug.to_ascii_lowercase(),
				name: listed.name,
				headquarters: listed.headquarters,
				datacenters: listed.datacenters,
			})
			.collect();
		let guardrail = match (provider.api_key.as_deref(), provider.management_key.as_deref()) {
			(Some(api_key), Some(management_key)) => key_guardrail(provider, api_key, management_key).await,
			(Some(_), None) => UpstreamGuardrail::NeedsManagementKey,
			(None, _) => UpstreamGuardrail::NotChecked,
		};
		Ok(UpstreamSnapshot { providers, guardrail })
	}
}

async fn key_guardrail(provider: &SourceProvider, api_key: &str, management_key: &str) -> UpstreamGuardrail {
	let base_url = &provider.base_url;
	let headers = &provider.extra_headers;
	let loaded = async {
		let current = omniference::catalog::fetch_current_key(base_url, api_key, headers, Some(TIMEOUT_MS)).await?;
		let keys = omniference::catalog::fetch_api_keys(base_url, management_key, headers, Some(TIMEOUT_MS)).await?;
		let Some(hash) = own_key_hash(api_key, current.label.as_deref(), &keys) else {
			return Ok(None);
		};
		let assignments = omniference::catalog::fetch_guardrail_key_assignments(base_url, management_key, headers, Some(TIMEOUT_MS)).await?;
		let guardrails = omniference::catalog::fetch_guardrails(base_url, management_key, headers, Some(TIMEOUT_MS)).await?;
		Ok::<_, omniference::adapter::AdapterError>(Some(guardrail_for(&hash, &assignments, &guardrails)))
	}
	.await;
	match loaded {
		Ok(Some(guardrail)) => guardrail,
		Ok(None) => UpstreamGuardrail::KeyNotFound,
		Err(error) => {
			tracing::warn!(%error, "OpenRouter guardrail lookup failed");
			UpstreamGuardrail::Unavailable
		}
	}
}

/// The account's hash of the inference key: its SHA-256 if the account lists it, else the only key with the same label.
#[must_use]
pub fn own_key_hash(api_key: &str, label: Option<&str>, keys: &[OpenRouterApiKey]) -> Option<String> {
	let digest = format!("{:x}", Sha256::digest(api_key.as_bytes()));
	if let Some(key) = keys.iter().find(|key| key.hash.eq_ignore_ascii_case(&digest)) {
		return Some(key.hash.clone());
	}
	let label = label?;
	let mut same_label = keys.iter().filter(|key| key.label.as_deref() == Some(label));
	match (same_label.next(), same_label.next()) {
		(Some(key), None) => Some(key.hash.clone()),
		_ => None,
	}
}

/// The guardrail assigned to the key with `hash`, or [`UpstreamGuardrail::None`] when it has none.
#[must_use]
pub fn guardrail_for(hash: &str, assignments: &[OpenRouterKeyAssignment], guardrails: &[OpenRouterGuardrail]) -> UpstreamGuardrail {
	let Some(assignment) = assignments.iter().find(|assignment| assignment.key_hash.eq_ignore_ascii_case(hash)) else {
		return UpstreamGuardrail::None;
	};
	let Some(guardrail) = guardrails.iter().find(|guardrail| guardrail.id == assignment.guardrail_id) else {
		return UpstreamGuardrail::Unavailable;
	};
	let lower = |slugs: &[String]| slugs.iter().map(|slug| slug.to_ascii_lowercase()).collect::<Vec<_>>();
	UpstreamGuardrail::Assigned {
		name: guardrail.name.clone(),
		allowed_providers: guardrail.allowed_providers.as_deref().map(lower),
		ignored_providers: guardrail.ignored_providers.as_deref().map(lower).unwrap_or_default(),
		allowed_data_regions: guardrail.allowed_data_regions.clone().unwrap_or_default(),
	}
}
