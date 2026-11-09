use super::sources::{SourceProvider, UpstreamSnapshot, UpstreamSyncError, source_for};
use super::{UpstreamGatewayResponse, UpstreamGuardrail, UpstreamProviderResponse, UpstreamSyncResponse};
use crate::ai::parse_extra_headers;
use crate::config::Config;
use crate::types::providers::{Provider, ProviderKind};
use crate::utils::encryption::decrypt_api_key;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::types::Json;

type StoredProvider = (String, Option<String>, Option<String>, Vec<String>);

impl UpstreamGatewayResponse {
	/// Every routing provider kind the installation uses, with the upstream providers it can send requests to.
	///
	/// A kind is listed when an enabled provider of that kind is configured, it was synced, or the installation
	/// has an allowlist for it. Its providers come from the last sync, else from the synced model endpoint options.
	///
	/// # Errors
	/// Returns an error when a database query fails.
	pub async fn load_all(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		let configured = Provider::list_enabled_system(pool).await?;
		let allowlists = Config::get().provider_routing().allowed_providers;
		let synced = sqlx::query_as::<_, (ProviderKind, DateTime<Utc>, Json<UpstreamGuardrail>)>("SELECT kind, synced_at, guardrail FROM upstream_gateways")
			.fetch_all(pool)
			.await?;
		let mut gateways = Vec::with_capacity(ProviderKind::UPSTREAM_ROUTING.len());
		for kind in ProviderKind::UPSTREAM_ROUTING {
			let snapshot = synced.iter().find(|(synced_kind, ..)| synced_kind == kind);
			let provider_id = configured.iter().find(|provider| &provider.kind == kind).map(|provider| provider.id);
			if snapshot.is_none() && provider_id.is_none() && !allowlists.contains_key(kind) {
				continue;
			}
			let mut gateway = match snapshot {
				Some((_, synced_at, Json(guardrail))) => Self::stored(pool, kind, *synced_at, guardrail.clone()).await?,
				None => Self::from_endpoint_options(pool, kind).await?,
			};
			gateway.provider_id = provider_id;
			gateways.push(gateway);
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

	async fn stored(pool: &PgPool, kind: &ProviderKind, synced_at: DateTime<Utc>, guardrail: UpstreamGuardrail) -> Result<Self, sqlx::Error> {
		let rows = sqlx::query_as::<_, StoredProvider>("SELECT slug, name, headquarters, datacenters FROM upstream_providers WHERE kind = $1 ORDER BY slug")
			.bind(kind)
			.fetch_all(pool)
			.await?;
		let providers = rows
			.into_iter()
			.map(|(slug, name, headquarters, datacenters)| UpstreamProviderResponse {
				reachable: guardrail.permits(&slug),
				slug,
				name,
				headquarters,
				datacenters,
			})
			.collect();
		Ok(Self { kind: kind.clone(), provider_id: None, synced_at: Some(synced_at), guardrail, providers })
	}

	async fn from_endpoint_options(pool: &PgPool, kind: &ProviderKind) -> Result<Self, sqlx::Error> {
		let rows = sqlx::query_as::<_, (String, Option<String>)>(
			r#"
			SELECT split_part(LOWER(o.provider_slug), '/', 1) AS slug, MAX(o.provider_name) AS name
			FROM gateway_model_provider_options o
			JOIN gateway_catalog_models c ON c.id = o.catalog_model_id
			JOIN providers p ON p.id = c.provider_id
			WHERE p.kind = $1 AND o.provider_slug IS NOT NULL AND o.provider_slug <> ''
			GROUP BY 1
			ORDER BY 1
			"#,
		)
		.bind(kind)
		.fetch_all(pool)
		.await?;
		let providers = rows
			.into_iter()
			.map(|(slug, name)| UpstreamProviderResponse { slug, name, headquarters: None, datacenters: Vec::new(), reachable: true })
			.collect();
		Ok(Self { kind: kind.clone(), provider_id: None, synced_at: None, guardrail: UpstreamGuardrail::NotChecked, providers })
	}
}

/// Syncs one routing provider through its kind's source; `false` when it could not be reached. Kinds without a source are skipped.
async fn sync_one(pool: &PgPool, provider: &Provider) -> Result<bool, sqlx::Error> {
	let Some(source) = source_for(&provider.kind) else {
		return Ok(true);
	};
	let management = Provider::find_billing_connection_for_admin(pool, &provider.id).await?.and_then(|connection| connection.credential);
	let result = match source_provider(provider, management.as_deref()) {
		Ok(credentials) => source.sync(&credentials).await,
		Err(error) => Err(error),
	};
	match result {
		Ok(snapshot) => {
			store(pool, &provider.kind, &snapshot).await?;
			Ok(true)
		}
		Err(error) => {
			tracing::warn!(kind = ?provider.kind, %error, "upstream provider sync failed");
			Ok(false)
		}
	}
}

/// The routing provider's address and decrypted credentials, including the management key stored for billing.
fn source_provider(provider: &Provider, management_key: Option<&str>) -> Result<SourceProvider, UpstreamSyncError> {
	Ok(SourceProvider {
		base_url: provider.base_url.clone(),
		api_key: provider.api_key.as_deref().map(decrypt_api_key).transpose().map_err(|_| UpstreamSyncError::Credential)?,
		management_key: management_key.map(decrypt_api_key).transpose().map_err(|_| UpstreamSyncError::Credential)?,
		extra_headers: parse_extra_headers(&provider.extra_headers.0).map_err(|_| UpstreamSyncError::Credential)?,
	})
}

/// Replaces a kind's stored snapshot in one transaction.
async fn store(pool: &PgPool, kind: &ProviderKind, snapshot: &UpstreamSnapshot) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;
	sqlx::query(
		r#"
		INSERT INTO upstream_gateways (kind, synced_at, guardrail) VALUES ($1, NOW(), $2)
		ON CONFLICT (kind) DO UPDATE SET synced_at = NOW(), guardrail = EXCLUDED.guardrail
		"#,
	)
	.bind(kind)
	.bind(Json(&snapshot.guardrail))
	.execute(&mut *tx)
	.await?;
	sqlx::query("DELETE FROM upstream_providers WHERE kind = $1").bind(kind).execute(&mut *tx).await?;
	let count = snapshot.providers.len();
	let (mut slugs, mut names, mut headquarters, mut datacenters) = (Vec::with_capacity(count), Vec::with_capacity(count), Vec::with_capacity(count), Vec::with_capacity(count));
	for provider in &snapshot.providers {
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
