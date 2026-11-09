//! Sources that sync the upstream providers of a routing provider kind.
//!
//! Every kind in [`ProviderKind::UPSTREAM_ROUTING`] may have a source. A source asks the routing provider which
//! upstream providers exist. Kinds without a source fall back to
//! the provider names seen in the synced model endpoint options. To add one (e.g. Vercel or Cortecs), add a file
//! with its `sync` function and call it from [`sync`].

mod openrouter;

use crate::types::providers::ProviderKind;
use std::collections::BTreeMap;

/// Credentials and address of the routing provider a source talks to.
pub struct SourceProvider {
	pub base_url: String,
	pub api_key: Option<String>,
	pub extra_headers: BTreeMap<String, String>,
}

/// An upstream provider as a source reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedUpstreamProvider {
	pub slug: String,
	pub name: Option<String>,
	pub headquarters: Option<String>,
	pub datacenters: Vec<String>,
}

/// Why a sync failed; the previous snapshot is kept then.
#[derive(Debug, thiserror::Error)]
pub enum UpstreamSyncError {
	#[error("the provider credential could not be decrypted")]
	Credential,
	#[error("the routing provider could not be reached: {0}")]
	Request(String),
}

/// Fetches the upstream providers of a routing provider; `None` when its kind has no source.
pub async fn sync(kind: &ProviderKind, provider: &SourceProvider) -> Option<Result<Vec<SyncedUpstreamProvider>, UpstreamSyncError>> {
	match kind {
		ProviderKind::Openrouter => Some(openrouter::sync(provider).await),
		_ => None,
	}
}
