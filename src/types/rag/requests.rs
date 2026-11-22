use super::{ConnectionKind, ConnectionMode, RagError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The largest vector `halfvec` can index with HNSW.
pub const MAX_DIMENSIONS: u32 = 4000;

/// Creates a connection; an update takes the same shape and ignores `kind` and `mode`.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveConnectionRequest {
	pub name: String,
	pub kind: ConnectionKind,
	pub mode: ConnectionMode,
	/// `None` on a managed pgvector connection means the main database. Never contains a password.
	pub url: Option<String>,
	/// Postgres password or Qdrant API key. `None` keeps the stored one, an empty string removes it.
	pub secret: Option<String>,
	/// Managed connections only.
	pub embedding: Option<EmbeddingSettings>,
}

/// The model every knowledge base of a managed connection is indexed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EmbeddingSettings {
	pub model_id: Uuid,
	/// Asked from the model; `None` takes its default size.
	pub dimensions: Option<u32>,
}

/// Saves the installation's knowledge settings; `GET` returns the same objects.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UpdateRagSetupRequest {
	pub enabled: bool,
	pub reranker: RerankerSettings,
	pub defaults: RagDefaults,
}

/// Optional model that sorts search hits by relevance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RerankerSettings {
	pub model_id: Option<Uuid>,
	/// Hits kept after reranking.
	pub top_n: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RagDefaults {
	/// Managed connection new knowledge bases are stored in.
	pub connection_id: Option<Uuid>,
}

impl SaveConnectionRequest {
	/// The trimmed URL, or `None` when it is empty.
	#[must_use]
	pub fn url(&self) -> Option<&str> {
		self.url.as_deref().map(str::trim).filter(|url| !url.is_empty())
	}

	/// Whether the request types a new secret instead of keeping the stored one.
	#[must_use]
	pub fn sets_secret(&self) -> bool {
		self.secret.as_deref().is_some_and(|secret| !secret.is_empty())
	}

	/// Checks the fields that need no database; `kind` and `mode` are the connection's own on an update.
	///
	/// # Errors
	///
	/// Returns the first problem found.
	pub fn validate(&self, kind: ConnectionKind, mode: ConnectionMode, keeps_secret: bool) -> Result<(), RagError> {
		if !(1..=100).contains(&self.name.trim().chars().count()) {
			return Err(RagError::InvalidName);
		}
		if mode == ConnectionMode::Managed && kind != ConnectionKind::Pgvector {
			return Err(RagError::ManagedNeedsPgvector);
		}
		match (self.url(), mode) {
			(None, ConnectionMode::ReadOnly) => return Err(RagError::UrlRequired),
			(None, ConnectionMode::Managed) => {}
			(Some(url), _) => validate_url(url, kind, self.sets_secret() || keeps_secret)?,
		}
		match (&self.embedding, mode) {
			(Some(_), ConnectionMode::ReadOnly) => Err(RagError::ReadOnlyHasModel),
			(
				Some(EmbeddingSettings {
					dimensions: Some(dimensions), ..
				}),
				_,
			) if !(1..=MAX_DIMENSIONS).contains(dimensions) => Err(RagError::InvalidDimensions),
			_ => Ok(()),
		}
	}
}

fn validate_url(url: &str, kind: ConnectionKind, has_secret: bool) -> Result<(), RagError> {
	let parsed = reqwest::Url::parse(url).map_err(|_| RagError::InvalidUrl)?;
	if parsed.host_str().is_none_or(str::is_empty) {
		return Err(RagError::InvalidUrl);
	}
	if parsed.password().is_some() || parsed.query_pairs().any(|(key, _)| key == "password") {
		return Err(RagError::UrlHasPassword);
	}
	if kind == ConnectionKind::Pgvector && (parsed.host_str().is_some_and(|host| host.contains('%')) || parsed.query_pairs().any(|(key, _)| key != "sslmode")) {
		return Err(RagError::InvalidUrl);
	}
	match (kind, parsed.scheme()) {
		(ConnectionKind::Pgvector, "postgres" | "postgresql") | (ConnectionKind::Qdrant, "https") => Ok(()),
		(ConnectionKind::Qdrant, "http") if has_secret => Err(RagError::InsecureSecret),
		(ConnectionKind::Qdrant, "http") => Ok(()),
		_ => Err(RagError::InvalidUrl),
	}
}

impl UpdateRagSetupRequest {
	/// Checks the fields that need no database.
	///
	/// # Errors
	///
	/// Returns `InvalidTopN` when `top_n` is outside 1 to 50.
	pub const fn validate(&self) -> Result<(), RagError> {
		if self.reranker.top_n < 1 || self.reranker.top_n > 50 {
			return Err(RagError::InvalidTopN);
		}
		Ok(())
	}
}
