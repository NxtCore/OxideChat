//! Upstream provider routing policy.
//!
//! Provider kinds that route requests on to upstream providers (see [`ProviderKind::routes_upstream`]) are
//! governed by an installation setting and per-team overrides: whether users get the chat provider selector,
//! and, per routing provider kind, which upstream providers requests may reach. [`ProviderRoutingMiddleware`]
//! enforces the allowlists on every request that goes through the engine, from web chat and from the gateway.

mod middleware;
mod repository;
pub mod sources;
mod upstream;

pub use middleware::ProviderRoutingMiddleware;
pub use upstream::sync_one as sync_upstream_providers;

use crate::types::providers::ProviderKind;
use omniference::types::ProviderRouting;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Most upstream provider slugs one allowlist may hold.
pub const MAX_ALLOWED_PROVIDERS: usize = 200;
const MAX_PROVIDER_SLUG_CHARS: usize = 64;

/// Allowed upstream provider slugs per routing provider kind; a kind without an entry allows every provider.
pub type ProviderAllowlists = BTreeMap<ProviderKind, Vec<String>>;

// ---------------------------------------------------------------------------
// Request / Response Types
// ---------------------------------------------------------------------------

/// Installation-wide routing settings; the admin PATCH takes the same shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProviderRoutingSettings {
	/// Whether users get the chat provider selector, unless their team says otherwise.
	pub selector_enabled: bool,
	/// Upstream providers requests may be routed to, per routing provider kind.
	#[serde(default)]
	pub allowed_providers: ProviderAllowlists,
}

/// A team's routing settings; a `None` selector and a kind without an allowlist follow the installation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TeamProviderRouting {
	/// Turns the chat provider selector on or off for the team's members.
	pub selector_enabled: Option<bool>,
	/// Upstream providers the team's requests may be routed to, per routing provider kind, within the
	/// installation's allowlists.
	#[serde(default)]
	pub allowed_providers: ProviderAllowlists,
}

/// The chat provider selector as the current user gets it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ChatProviderSelectorResponse {
	pub enabled: bool,
	/// Provider kinds whose models show the selector.
	pub provider_kinds: Vec<ProviderKind>,
}

/// An upstream provider a routing provider can send requests to, offered when editing an allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UpstreamProviderResponse {
	pub slug: String,
	pub name: Option<String>,
	/// ISO country code of the provider's headquarters, when the routing provider reports it.
	pub headquarters: Option<String>,
	/// ISO country codes of the provider's datacenters, when the routing provider reports them.
	pub datacenters: Vec<String>,
}

/// A routing provider kind with the upstream providers it can send requests to.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UpstreamGatewayResponse {
	pub kind: ProviderKind,
	/// The configured provider of this kind that is synced, if there is one.
	#[cfg_attr(test, ts(type = "string | null"))]
	pub provider_id: Option<uuid::Uuid>,
	/// When the providers were last synced from the routing provider; `None` when they come from the
	/// model endpoint options instead.
	#[cfg_attr(test, ts(type = "string | null"))]
	pub synced_at: Option<chrono::DateTime<chrono::Utc>>,
	pub providers: Vec<UpstreamProviderResponse>,
}

/// The result of syncing the upstream providers of every routing provider kind.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UpstreamSyncResponse {
	pub gateways: Vec<UpstreamGatewayResponse>,
	/// Kinds whose routing provider could not be reached; they keep their previous snapshot.
	pub failed: Vec<ProviderKind>,
}

// ---------------------------------------------------------------------------
// Internal Types
// ---------------------------------------------------------------------------

/// Settings that apply to one request after combining the installation and the subject's teams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveProviderRouting {
	pub selector_enabled: bool,
	/// A kind without an entry allows every provider; an entry with no slugs allows none.
	pub allowed_providers: ProviderAllowlists,
}

/// Why a request may not be routed as asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderRoutingError {
	#[error("upstream provider `{0}` is not allowed")]
	NotAllowed(String),
	#[error("no upstream provider is allowed")]
	NoneAllowed,
}

fn valid_slug(slug: &str) -> bool {
	!slug.is_empty() && slug.chars().count() <= MAX_PROVIDER_SLUG_CHARS && slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

/// Trims, lowercases, sorts and deduplicates an allowlist.
///
/// Returns `None` when a slug is invalid, the list is empty or it is too long.
#[must_use]
pub fn normalize_allowed_providers(providers: &[String]) -> Option<Vec<String>> {
	let mut slugs: Vec<String> = providers.iter().map(|slug| slug.trim().to_ascii_lowercase()).collect();
	slugs.sort_unstable();
	slugs.dedup();
	(!slugs.is_empty() && slugs.len() <= MAX_ALLOWED_PROVIDERS && slugs.iter().all(|slug| valid_slug(slug))).then_some(slugs)
}

/// Normalizes every allowlist; `None` when one is invalid or belongs to a kind that does not route upstream.
#[must_use]
pub fn normalize_allowlists(lists: &ProviderAllowlists) -> Option<ProviderAllowlists> {
	lists
		.iter()
		.map(|(kind, list)| if kind.routes_upstream() { normalize_allowed_providers(list).map(|list| (kind.clone(), list)) } else { None })
		.collect()
}

/// Whether a requested slug, which may be a variant such as `deepinfra/fp8`, falls under an allowed slug.
fn slug_matches(allowed: &str, requested: &str) -> bool {
	let requested = requested.trim();
	if requested.eq_ignore_ascii_case(allowed) {
		return true;
	}
	requested.get(..allowed.len()).is_some_and(|base| base.eq_ignore_ascii_case(allowed)) && requested[allowed.len()..].starts_with('/')
}

/// The slugs both allowlists allow, keeping the narrower slug when one is a variant of the other.
fn intersect(left: &[String], right: &[String]) -> Vec<String> {
	let mut slugs: Vec<String> = left
		.iter()
		.flat_map(|a| right.iter().map(move |b| (a, b)))
		.filter_map(|(a, b)| if slug_matches(a, b) { Some(b.clone()) } else { slug_matches(b, a).then(|| a.clone()) })
		.collect();
	slugs.sort_unstable();
	slugs.dedup();
	slugs
}

impl ProviderRoutingSettings {
	/// The same settings with normalized allowlists, or `None` when one is invalid.
	#[must_use]
	pub fn normalized(&self) -> Option<Self> {
		Some(Self {
			selector_enabled: self.selector_enabled,
			allowed_providers: normalize_allowlists(&self.allowed_providers)?,
		})
	}
}

impl TeamProviderRouting {
	/// The same settings with normalized allowlists, or `None` when one is invalid.
	#[must_use]
	pub fn normalized(&self) -> Option<Self> {
		Some(Self {
			selector_enabled: self.selector_enabled,
			allowed_providers: normalize_allowlists(&self.allowed_providers)?,
		})
	}
}

impl EffectiveProviderRouting {
	/// Combines the installation with every team of the subject, keeping the most restrictive allowlist per kind.
	///
	/// A team that turns the selector off wins over one that turns it on; with no team override the
	/// installation decides.
	#[must_use]
	pub fn combine(installation: &ProviderRoutingSettings, teams: &[TeamProviderRouting]) -> Self {
		let selector_enabled = if teams.iter().any(|team| team.selector_enabled == Some(false)) {
			false
		} else {
			installation.selector_enabled || teams.iter().any(|team| team.selector_enabled == Some(true))
		};
		let mut allowed_providers = installation.allowed_providers.clone();
		for (kind, list) in teams.iter().flat_map(|team| &team.allowed_providers) {
			allowed_providers
				.entry(kind.clone())
				.and_modify(|allowed| *allowed = intersect(allowed, list))
				.or_insert_with(|| list.clone());
		}
		Self { selector_enabled, allowed_providers }
	}

	/// Whether requests through a provider of `kind` may be routed to the upstream provider `slug`.
	#[must_use]
	pub fn allows(&self, kind: &ProviderKind, slug: &str) -> bool {
		self.allowed_providers.get(kind).is_none_or(|allowed| allowed.iter().any(|allowed| slug_matches(allowed, slug)))
	}

	/// Whether requests through a provider of `kind` are limited to an allowlist.
	#[must_use]
	pub fn is_restricted(&self, kind: &ProviderKind) -> bool {
		self.allowed_providers.contains_key(kind)
	}

	/// Narrows a request's routing through a provider of `kind` to the allowed providers.
	///
	/// Without an allowlist for `kind` the routing is returned as is. Otherwise every provider the request
	/// names must be allowed, and a request that names no allowlist of its own is restricted to the allowed providers.
	///
	/// # Errors
	/// Returns [`ProviderRoutingError`] when the request names a provider that is not allowed, or when no
	/// provider is allowed at all.
	pub fn restrict(&self, kind: &ProviderKind, routing: Option<ProviderRouting>) -> Result<Option<ProviderRouting>, ProviderRoutingError> {
		let Some(allowed) = self.allowed_providers.get(kind) else {
			return Ok(routing);
		};
		if allowed.is_empty() {
			return Err(ProviderRoutingError::NoneAllowed);
		}
		let mut routing = routing.unwrap_or_default();
		if let Some(slug) = routing.order.iter().chain(routing.only.iter()).flatten().find(|slug| !self.allows(kind, slug)) {
			return Err(ProviderRoutingError::NotAllowed(slug.clone()));
		}
		if routing.only.as_ref().is_none_or(Vec::is_empty) {
			routing.only = Some(allowed.clone());
		}
		Ok(Some(routing))
	}
}
