use super::sources::{self, SourceProvider, SyncedUpstreamProvider, UpstreamSyncError};
use super::{UpstreamGatewayResponse, UpstreamProviderResponse, UpstreamSyncResponse};
use crate::ai::parse_extra_headers;
use crate::config::Config;
use crate::types::providers::{Provider, ProviderKind};
use crate::utils::encryption::decrypt_api_key;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

type StoredProvider = (String, Option<String>, Option<String>, Vec<String>);

impl UpstreamGatewayResponse {
	/// Every routing provider kind the installation uses, with the upstream providers it can send requests to.
	///
	/// A kind is listed when an enabled provider of that kind is configured, it was synced, or the installation
	/// has an allowlist for it. Its providers are the last sync's plus every endpoint tag seen in the synced
	/// models, which adds variants the provider list leaves out, such as `google-vertex/europe`.
	///
	/// # Errors
	/// Returns an error when a database query fails.
	pub async fn load_all(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		let configured = Provider::list_enabled_system(pool).await?;
		let allowlists = Config::get().provider_routing().allowed_providers;
		let synced = sqlx::query_as::<_, (ProviderKind, DateTime<Utc>)>("SELECT kind, synced_at FROM upstream_gateways")
			.fetch_all(pool)
			.await?;
		let mut gateways = Vec::with_capacity(ProviderKind::UPSTREAM_ROUTING.len());
		for kind in ProviderKind::UPSTREAM_ROUTING {
			let synced_at = synced.iter().find(|(synced_kind, _)| synced_kind == kind).map(|(_, synced_at)| *synced_at);
			let provider_id = configured.iter().find(|provider| &provider.kind == kind).map(|provider| provider.id);
			if synced_at.is_none() && provider_id.is_none() && !allowlists.contains_key(kind) {
				continue;
			}
			let mut providers = stored_providers(pool, kind).await?;
			providers.extend(endpoint_providers(pool, kind).await?);
			providers.sort_by(|a, b| a.slug.cmp(&b.slug));
			providers.dedup_by(|endpoint, synced| endpoint.slug == synced.slug);
			gateways.push(Self { kind: kind.clone(), provider_id, synced_at, providers });
		}
		Ok(gateways)
	}

	/// Syncs every routing provider kind that has a source and an enabled provider, then lists all gateways.
	///
	/// A kind whose routing provider cannot be reached keeps its previous snapshot and is reported in `failed`.
	///
	/// # Errors
	/// Returns an error when a database query fails.
	pub async fn sync_all(pool: &PgPool) -> Result<UpstreamSyncResponse, sqlx::Error> {
		let configured = Provider::list_enabled_system(pool).await?;
		let mut failed = Vec::new();
		for kind in ProviderKind::UPSTREAM_ROUTING {
			if let Some(provider) = configured.iter().find(|provider| &provider.kind == kind)
				&& !sync_one(pool, provider).await?
			{
				failed.push(kind.clone());
			}
		}
		Ok(UpstreamSyncResponse { gateways: Self::load_all(pool).await?, failed })
	}
}

async fn stored_providers(pool: &PgPool, kind: &ProviderKind) -> Result<Vec<UpstreamProviderResponse>, sqlx::Error> {
	let rows = sqlx::query_as::<_, StoredProvider>("SELECT slug, name, headquarters, datacenters FROM upstream_providers WHERE kind = $1")
		.bind(kind)
		.fetch_all(pool)
		.await?;
	Ok(rows
		.into_iter()
		.map(|(slug, name, headquarters, datacenters)| UpstreamProviderResponse { slug, name, headquarters, datacenters })
		.collect())
}

async fn endpoint_providers(pool: &PgPool, kind: &ProviderKind) -> Result<Vec<UpstreamProviderResponse>, sqlx::Error> {
	let rows = sqlx::query_as::<_, (String, Option<String>)>(
		r#"
		SELECT LOWER(o.provider_slug) AS slug, MAX(o.provider_name) AS name
		FROM gateway_model_provider_options o
		JOIN gateway_catalog_models c ON c.id = o.catalog_model_id
		JOIN providers p ON p.id = c.provider_id
		WHERE p.kind = $1 AND o.provider_slug IS NOT NULL AND o.provider_slug <> ''
		GROUP BY 1
		"#,
	)
	.bind(kind)
	.fetch_all(pool)
	.await?;
	Ok(rows
		.into_iter()
		.map(|(slug, name)| UpstreamProviderResponse { slug, name, headquarters: None, datacenters: Vec::new() })
		.collect())
}

/// Syncs one routing provider through its kind's source; `false` when it could not be reached. Kinds without a source are skipped.
/// Syncs the upstream providers of one routing provider; `false` when it could not be reached, which keeps the
/// previous snapshot.
///
/// # Errors
/// Returns an error when storing the snapshot fails.
pub async fn sync_one(pool: &PgPool, provider: &Provider) -> Result<bool, sqlx::Error> {
	let result = match source_provider(provider) {
		Ok(credentials) => match sources::sync(&provider.kind, &credentials).await {
			Some(result) => result,
			None => return Ok(true),
		},
		Err(error) => Err(error),
	};
	match result {
		Ok(providers) => {
			store(pool, &provider.kind, &providers).await?;
			Ok(true)
		}
		Err(error) => {
			tracing::warn!(kind = ?provider.kind, %error, "upstream provider sync failed");
			Ok(false)
		}
	}
}

/// The routing provider's address and decrypted credentials.
fn source_provider(provider: &Provider) -> Result<SourceProvider, UpstreamSyncError> {
	Ok(SourceProvider {
		base_url: provider.base_url.clone(),
		api_key: provider.api_key.as_deref().map(decrypt_api_key).transpose().map_err(|_| UpstreamSyncError::Credential)?,
		extra_headers: parse_extra_headers(&provider.extra_headers.0).map_err(|_| UpstreamSyncError::Credential)?,
	})
}

/// Replaces a kind's stored snapshot in one transaction.
async fn store(pool: &PgPool, kind: &ProviderKind, providers: &[SyncedUpstreamProvider]) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;
	sqlx::query(
		r#"
		INSERT INTO upstream_gateways (kind, synced_at) VALUES ($1, NOW())
		ON CONFLICT (kind) DO UPDATE SET synced_at = NOW()
		"#,
	)
	.bind(kind)
	.execute(&mut *tx)
	.await?;
	sqlx::query("DELETE FROM upstream_providers WHERE kind = $1").bind(kind).execute(&mut *tx).await?;
	let count = providers.len();
	let (mut slugs, mut names, mut headquarters, mut datacenters) = (Vec::with_capacity(count), Vec::with_capacity(count), Vec::with_capacity(count), Vec::with_capacity(count));
	for provider in providers {
		slugs.push(provider.slug.as_str());
		names.push(provider.name.as_deref());
		headquarters.push(provider.headquarters.as_deref());
		datacenters.push(provider.datacenters.join(","));
	}
	sqlx::query(
		r#"
		INSERT INTO upstream_providers (kind, slug, name, headquarters, datacenters)
		SELECT $1, slug, name, headquarters, COALESCE(string_to_array(NULLIF(datacenters, ''), ','), '{}')
		FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[]) AS t(slug, name, headquarters, datacenters)
		ON CONFLICT (kind, slug) DO NOTHING
		"#,
	)
	.bind(kind)
	.bind(&slugs)
	.bind(&names)
	.bind(&headquarters)
	.bind(&datacenters)
	.execute(&mut *tx)
	.await?;
	tx.commit().await
}
