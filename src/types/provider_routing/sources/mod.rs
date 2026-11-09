//! Sources that sync the upstream providers of a routing provider kind.
//!
//! Every kind in [`ProviderKind::UPSTREAM_ROUTING`] may have a source. A source asks the routing provider which
//! upstream providers exist and which of them the configured key may reach. Kinds without a source fall back to
//! the provider names seen in the synced model endpoint options. To add one (e.g. Vercel or Cortecs), implement
//! [`UpstreamSource`] in its own file and return it from [`source_for`].

pub mod openrouter;

use super::UpstreamGuardrail;
use crate::types::providers::ProviderKind;
use async_trait::async_trait;
use std::collections::BTreeMap;

/// Credentials and address of the routing provider a source talks to.
pub struct SourceProvider {
	pub base_url: String,
	pub api_key: Option<String>,
	/// An account-level key some routing providers need to read guardrails (`OpenRouter`'s management key).
	pub management_key: Option<String>,
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

/// What one sync of a routing provider found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpstreamSnapshot {
	pub providers: Vec<SyncedUpstreamProvider>,
	pub guardrail: UpstreamGuardrail,
}

/// Why a sync failed; the previous snapshot is kept then.
#[derive(Debug, thiserror::Error)]
pub enum UpstreamSyncError {
	#[error("the provider credential could not be decrypted")]
	Credential,
	#[error("the routing provider could not be reached: {0}")]
	Request(String),
}

/// Lists the upstream providers of one routing provider kind.
#[async_trait]
pub trait UpstreamSource: Send + Sync {
	/// Fetches the upstream providers and the restrictions on the configured key.
	///
	/// # Errors
	/// Returns [`UpstreamSyncError`] when the provider list cannot be fetched. Restrictions that cannot be read
	/// are reported in [`UpstreamSnapshot::guardrail`] instead of failing the sync.
	async fn sync(&self, provider: &SourceProvider) -> Result<UpstreamSnapshot, UpstreamSyncError>;
}

/// The source for a routing provider kind, if it has one.
#[must_use]
pub fn source_for(kind: &ProviderKind) -> Option<&'static dyn UpstreamSource> {
	match kind {
		ProviderKind::Openrouter => Some(&openrouter::OpenRouterSource),
		_ => None,
	}
}
