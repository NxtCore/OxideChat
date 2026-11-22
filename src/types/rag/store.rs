//! Read access to the vector stores behind connections.

use super::{ConnectionKind, ConnectionMode, RagConnection};
use crate::utils::encryption::decrypt_api_key;
use crate::utils::outbound::{guarded_client, is_allowed_ip, private_hosts_allowed, read_limited};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::borrow::Cow;
use std::str::FromStr;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_COLLECTIONS: i64 = 500;
const MIN_PGVECTOR: (u32, u32) = (0, 8);

/// Why a vector store could not be used; translated as `rag.store_error.<value>` and never carries the server's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum StoreError {
	#[error("invalid url")]
	InvalidUrl,
	#[error("the address is not allowed")]
	Blocked,
	#[error("the server could not be reached")]
	Unreachable,
	#[error("the server refused the credentials")]
	Unauthorized,
	#[error("pgvector is not installed")]
	ExtensionMissing,
	#[error("pgvector is too old")]
	ExtensionTooOld,
	#[error("the server sent an unreadable answer")]
	BadResponse,
	#[error("the stored secret could not be decrypted")]
	Secret,
}

/// An open vector store; the main database is borrowed, every other store is its own pool or client.
pub enum VectorStore<'a> {
	Pgvector(Cow<'a, PgPool>),
	Qdrant(Qdrant),
}

/// A Qdrant server reached over REST.
pub struct Qdrant {
	client: reqwest::Client,
	base: reqwest::Url,
	api_key: Option<String>,
}

#[derive(Deserialize)]
struct QdrantRoot {
	version: String,
}

#[derive(Deserialize)]
struct QdrantCollections {
	result: QdrantCollectionList,
}

#[derive(Deserialize)]
struct QdrantCollectionList {
	collections: Vec<QdrantCollection>,
}

#[derive(Deserialize)]
struct QdrantCollection {
	name: String,
}

impl<'a> VectorStore<'a> {
	/// Opens the store of `connection`. `env_url` replaces its address and password (`RAG_PGVECTOR_URL`).
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the address is not allowed, the secret cannot be decrypted or the server cannot be reached.
	pub async fn connect(connection: &RagConnection, main: &'a PgPool, env_url: Option<&str>) -> Result<Self, StoreError> {
		let secret = connection.secret.as_deref().map(decrypt_api_key).transpose().map_err(|_| StoreError::Secret)?;
		match connection.kind {
			ConnectionKind::Pgvector => match (env_url, connection.url.as_deref()) {
				(Some(url), _) => connect_postgres(url, None, connection.mode, true).await,
				(None, Some(url)) => connect_postgres(url, secret.as_deref(), connection.mode, false).await,
				(None, None) => Ok(Self::Pgvector(Cow::Borrowed(main))),
			},
			ConnectionKind::Qdrant => {
				let url = connection.url.as_deref().ok_or(StoreError::InvalidUrl)?;
				let base = if url.ends_with('/') { Cow::Borrowed(url) } else { Cow::Owned(format!("{url}/")) };
				let (client, base) = guarded_client(&base).await.map_err(|error| match error {
					crate::utils::outbound::OutboundError::Blocked => StoreError::Blocked,
					crate::utils::outbound::OutboundError::Lookup => StoreError::Unreachable,
					_ => StoreError::InvalidUrl,
				})?;
				if secret.is_some() && base.scheme() != "https" {
					return Err(StoreError::InvalidUrl);
				}
				Ok(Self::Qdrant(Qdrant { client, base, api_key: secret }))
			}
		}
	}

	/// The pgvector extension or Qdrant server version.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be reached, refuses the credentials or lacks pgvector 0.8.
	pub async fn health(&self) -> Result<String, StoreError> {
		match self {
			Self::Pgvector(pool) => {
				let version: Option<String> =
					timed(sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'").fetch_optional(pool.as_ref())).await?;
				let version = version.ok_or(StoreError::ExtensionMissing)?;
				if parse_version(&version) < MIN_PGVECTOR {
					return Err(StoreError::ExtensionTooOld);
				}
				Ok(version)
			}
			Self::Qdrant(qdrant) => Ok(qdrant.get::<QdrantRoot>("").await?.version),
		}
	}

	/// Tables with a vector column (`schema.table`) or Qdrant collections.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be reached or answers unreadably.
	pub async fn list_collections(&self) -> Result<Vec<String>, StoreError> {
		match self {
			Self::Pgvector(pool) => {
				let query = sqlx::query_scalar(
					"SELECT DISTINCT format('%I.%I', table_schema, table_name) AS name FROM information_schema.columns
					 WHERE udt_name IN ('vector', 'halfvec') AND table_schema NOT IN ('pg_catalog', 'information_schema')
					 ORDER BY name LIMIT $1",
				)
				.bind(MAX_COLLECTIONS)
				.fetch_all(pool.as_ref());
				timed(query).await
			}
			Self::Qdrant(qdrant) => {
				let mut names: Vec<String> = qdrant
					.get::<QdrantCollections>("collections")
					.await?
					.result
					.collections
					.into_iter()
					.map(|collection| collection.name)
					.collect();
				names.sort();
				Ok(names)
			}
		}
	}
}

async fn connect_postgres(url: &str, password: Option<&str>, mode: ConnectionMode, trusted: bool) -> Result<VectorStore<'static>, StoreError> {
	let mut options = PgConnectOptions::from_str(url).map_err(|_| StoreError::InvalidUrl)?;
	if !trusted {
		check_host(options.get_host(), options.get_port()).await?;
	}
	if let Some(password) = password {
		options = options.password(password);
	}
	if mode == ConnectionMode::ReadOnly {
		options = options.options([("default_transaction_read_only", "on")]);
	}
	let pool = PgPoolOptions::new()
		.max_connections(2)
		.acquire_timeout(TIMEOUT)
		.connect_with(options)
		.await
		.map_err(|error| store_error(&error))?;
	Ok(VectorStore::Pgvector(Cow::Owned(pool)))
}

// shortcut: sqlx resolves the host again when it connects, so a DNS answer that changes in between is not caught; pin the address once sqlx can keep TLS host verification with it.
async fn check_host(host: &str, port: u16) -> Result<(), StoreError> {
	if host.is_empty() || host.starts_with('/') {
		return Err(StoreError::InvalidUrl);
	}
	let addresses: Vec<_> = tokio::net::lookup_host((host.trim_matches(['[', ']']), port))
		.await
		.map_err(|_| StoreError::Unreachable)?
		.collect();
	let allow_private = private_hosts_allowed();
	if addresses.is_empty() || addresses.iter().any(|address| !is_allowed_ip(address.ip(), allow_private)) {
		return Err(StoreError::Blocked);
	}
	Ok(())
}

impl Qdrant {
	async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, StoreError> {
		let url = self.base.join(path).map_err(|_| StoreError::InvalidUrl)?;
		let mut request = self.client.get(url);
		if let Some(key) = &self.api_key {
			request = request.header("api-key", key);
		}
		let response = request.send().await.map_err(|_| StoreError::Unreachable)?;
		match response.status() {
			reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => return Err(StoreError::Unauthorized),
			status if !status.is_success() => return Err(StoreError::BadResponse),
			_ => {}
		}
		let body = read_limited(response, MAX_RESPONSE_BYTES).await.map_err(|_| StoreError::BadResponse)?;
		serde_json::from_slice(&body).map_err(|_| StoreError::BadResponse)
	}
}

async fn timed<T>(query: impl Future<Output = Result<T, sqlx::Error>>) -> Result<T, StoreError> {
	tokio::time::timeout(TIMEOUT, query)
		.await
		.map_err(|_| StoreError::Unreachable)?
		.map_err(|error| store_error(&error))
}

fn store_error(error: &sqlx::Error) -> StoreError {
	let code = error.as_database_error().and_then(|error| error.code().map(Cow::into_owned));
	match code.as_deref() {
		Some("28P01" | "28000" | "42501") => StoreError::Unauthorized,
		_ if matches!(error, sqlx::Error::Configuration(_)) => StoreError::InvalidUrl,
		_ => StoreError::Unreachable,
	}
}

/// `major.minor` of a version such as `0.8.0`; unreadable parts count as 0.
#[must_use]
pub fn parse_version(version: &str) -> (u32, u32) {
	let mut parts = version.split('.').map(|part| part.parse().unwrap_or(0));
	(parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}
