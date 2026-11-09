use super::{SourceProvider, SyncedUpstreamProvider, UpstreamSyncError};

const TIMEOUT_MS: u64 = 15_000;

/// `OpenRouter`: the upstream providers from `/v1/providers`.
pub(super) async fn sync(provider: &SourceProvider) -> Result<Vec<SyncedUpstreamProvider>, UpstreamSyncError> {
	let listed = omniference::catalog::fetch_providers(&provider.base_url, provider.api_key.as_deref(), &provider.extra_headers, Some(TIMEOUT_MS))
		.await
		.map_err(|error| UpstreamSyncError::Request(error.to_string()))?;
	Ok(listed
		.into_iter()
		.map(|listed| SyncedUpstreamProvider {
			slug: listed.slug.to_ascii_lowercase(),
			name: listed.name,
			headquarters: listed.headquarters,
			datacenters: listed.datacenters,
		})
		.collect())
}
