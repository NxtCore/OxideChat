use super::{ConnectionKind, ConnectionMode, ExternalView, KnowledgeScope, ModelTargets, RagError, check_search_language};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The largest vector `halfvec` can index with HNSW.
pub const MAX_DIMENSIONS: u32 = 4000;

/// Chunking defaults for managed documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ChunkingSettings {
	pub chunk_size: u32,
	pub overlap_percent: u32,
}

impl Default for ChunkingSettings {
	fn default() -> Self {
		Self {
			chunk_size: 400,
			overlap_percent: 15,
		}
	}
}

/// Per-user upload rate and storage limits for managed documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UploadQuotaSettings {
	pub max_uploads_per_hour: u32,
	pub storage_quota_mb: Option<u32>,
}

impl Default for UploadQuotaSettings {
	fn default() -> Self {
		Self {
			max_uploads_per_hour: 60,
			storage_quota_mb: None,
		}
	}
}

/// Asks for chunk count, cost estimate and model-restriction conflicts before changing a managed connection's embedding model.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReindexEstimateRequest {
	pub embedding: EmbeddingSettings,
}

/// Filename query parameter when uploading a document to a managed knowledge base.
#[derive(Debug, Deserialize)]
pub struct DocumentUploadQuery {
	pub filename: String,
}

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
	pub reranker: RerankerSettings,
	pub defaults: RagDefaults,
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub chunking: Option<ChunkingSettings>,
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub quotas: Option<UploadQuotaSettings>,
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

/// Creates or changes a knowledge base (managed when `view` is omitted on create, external when `view` is given).
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveKnowledgeBaseRequest {
	pub name: String,
	#[serde(default)]
	pub description: Option<String>,
	/// On create of a managed base: `personal`, `team` or `installation`. Ignored on updates and external bases.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub scope: Option<KnowledgeScope>,
	/// Required when `scope` is `team`.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub team_id: Option<Uuid>,
	/// PostgreSQL full-text search configuration (`simple`, `english`, `german`, …) for managed bases.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub search_language: Option<String>,
	/// Required on create of an external base; `None` creates a managed base or keeps the stored source on update.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub view: Option<ExternalView>,
	/// On an update, replaces the model access in the same transaction; only knowledge admins may send it.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub model_access: Option<ModelTargets>,
}

/// Inspects a collection of a read-only connection, reading the sample with `filter`.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InspectRequest {
	pub collection: String,
	#[serde(default)]
	pub filter: Vec<super::FieldFilter>,
}

impl InspectRequest {
	/// Checks the collection name and the filter like a saved view.
	///
	/// # Errors
	///
	/// Returns `InvalidCollection` or `InvalidFilter`.
	pub fn validate(&self) -> Result<(), RagError> {
		super::view::validate_collection(&self.collection)?;
		super::view::validate_filter(&self.filter)
	}
}

/// The access label a label assignment route acts on.
#[derive(Debug, Deserialize)]
pub struct LabelQuery {
	pub label: String,
}

/// A test search of one knowledge base.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeSearchRequest {
	pub query: String,
	/// Search as this user (`admin.rag.edit`, audited): only the hits they would get.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub as_user: Option<Uuid>,
}

impl KnowledgeSearchRequest {
	/// The trimmed query.
	///
	/// # Errors
	///
	/// Returns `InvalidQuery` when it is empty or longer than 2000 characters.
	pub fn query(&self) -> Result<&str, RagError> {
		let query = self.query.trim();
		if query.is_empty() || query.chars().count() > 2000 {
			Err(RagError::InvalidQuery)
		} else {
			Ok(query)
		}
	}
}

impl SaveKnowledgeBaseRequest {
	/// The trimmed description, or `None` when it is empty.
	#[must_use]
	pub fn description(&self) -> Option<&str> {
		self.description.as_deref().map(str::trim).filter(|text| !text.is_empty())
	}

	/// Checks the fields that need no database or store.
	///
	/// # Errors
	///
	/// Returns the first problem found.
	pub fn validate(&self) -> Result<(), RagError> {
		if !(1..=100).contains(&self.name.trim().chars().count()) {
			return Err(RagError::InvalidName);
		}
		if self.description().is_some_and(|text| text.chars().count() > 2000) {
			return Err(RagError::InvalidDescription);
		}
		if let Some(language) = &self.search_language {
			check_search_language(language)?;
		}
		self.view.as_ref().map_or(Ok(()), ExternalView::validate)
	}
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
	/// Returns `InvalidTopN`, `InvalidChunking` or `InvalidQuotas`.
	pub fn validate(&self) -> Result<(), RagError> {
		if self.reranker.top_n < 1 || self.reranker.top_n > 50 {
			return Err(RagError::InvalidTopN);
		}
		if let Some(chunking) = self.chunking
			&& (!(100..=2000).contains(&chunking.chunk_size) || chunking.overlap_percent > 50)
		{
			return Err(RagError::InvalidChunking);
		}
		if let Some(quotas) = self.quotas
			&& (!(1..=10_000).contains(&quotas.max_uploads_per_hour) || quotas.storage_quota_mb.is_some_and(|mb| mb == 0))
		{
			return Err(RagError::InvalidQuotas);
		}
		Ok(())
	}
}
