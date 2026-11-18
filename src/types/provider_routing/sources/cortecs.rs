use super::{SourceProvider, SyncedUpstreamProvider, UpstreamSyncError};
use omniference::catalog::CortecsModel;
use std::collections::BTreeSet;

const TIMEOUT_MS: u64 = 15_000;

/// Cortecs: the union of every model's `providers` from `/v1/models?extended=true`. Cortecs has no
/// provider list and no display names, so the slug is the only field.
pub(super) async fn sync(provider: &SourceProvider) -> Result<Vec<SyncedUpstreamProvider>, UpstreamSyncError> {
	let models = omniference::catalog::fetch_cortecs_models(&provider.base_url, provider.api_key.as_deref(), &provider.extra_headers, Some(TIMEOUT_MS))
		.await
		.map_err(|error| UpstreamSyncError::Request(error.to_string()))?;
	Ok(upstream_providers(&models))
}

/// Every provider slug the models run on, lower case and without duplicates.
#[must_use]
pub fn upstream_providers(models: &[CortecsModel]) -> Vec<SyncedUpstreamProvider> {
	let slugs: BTreeSet<String> = models.iter().flat_map(|model| model.providers.iter()).map(|slug| slug.to_ascii_lowercase()).collect();
	slugs
		.into_iter()
		.map(|slug| SyncedUpstreamProvider {
			slug,
			name: None,
			headquarters: None,
			datacenters: Vec::new(),
		})
		.collect()
}
