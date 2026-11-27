use super::{
	ChunkingSettings, ConnectionKind, ConnectionMode, Distance, DocumentStatus, ExternalView, KnowledgeHit, KnowledgeScope, RagDefaults, RerankerSettings, StoreError,
	UploadQuotaSettings,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Everything the knowledge setup page shows.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RagSetupResponse {
	pub connections: Vec<ConnectionResponse>,
	pub reranker: RerankerSettings,
	pub defaults: RagDefaults,
	pub chunking: ChunkingSettings,
	pub quotas: UploadQuotaSettings,
}

/// Progress of an active re-index on a managed connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReindexProgress {
	pub done: u32,
	pub total: u32,
}

/// Preview shown to an admin before changing a managed connection's embedding model or dimensions.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReindexEstimateResponse {
	pub chunk_count: u64,
	pub document_count: u64,
	pub estimated_tokens: u64,
	pub estimated_cost_usd: Option<f64>,
	/// Knowledge bases on this connection whose model restrictions forbid the chosen embedding model.
	pub conflicting_knowledge_bases: Vec<String>,
}

/// A connection as admins see it; the secret is never returned.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ConnectionResponse {
	pub id: Uuid,
	pub name: String,
	pub kind: ConnectionKind,
	pub mode: ConnectionMode,
	/// `None` on a managed pgvector connection means the main database.
	pub url: Option<String>,
	pub has_secret: bool,
	/// `RAG_PGVECTOR_URL` replaces the address and password of this connection.
	pub url_from_env: bool,
	pub embedding: Option<ConnectionEmbedding>,
	/// Whether documents in this managed connection are still re-indexing to the current generation.
	pub reindexing: bool,
	pub reindex_progress: Option<ReindexProgress>,
}

/// The embedding model of a managed connection and the vector size its test embedding returned.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ConnectionEmbedding {
	/// `None` when the model was removed; the stored vectors keep their size.
	pub model_id: Option<Uuid>,
	pub dimensions: Option<u32>,
	pub vector_size: u32,
}

/// Result of testing a connection; a failure is data for the page, not an HTTP error.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ConnectionTestResponse {
	pub ok: bool,
	/// Translated as `rag.store_error.<error>`.
	pub error: Option<StoreError>,
	/// pgvector extension or Qdrant server version.
	pub version: Option<String>,
	/// Collections a read-only connection can search.
	pub collections: Vec<String>,
}

/// What a collection holds, for mapping its fields before a knowledge base is saved.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CollectionInfo {
	pub vectors: Vec<VectorInfo>,
	/// Payload paths found in the columns and sample records.
	pub fields: Vec<String>,
	/// A few records without their vectors; long strings are cut.
	pub sample: Vec<serde_json::Value>,
}

/// One vector of a collection.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct VectorInfo {
	/// Qdrant named vector or pgvector column; `None` is Qdrant's unnamed vector.
	pub name: Option<String>,
	/// `None` when the store does not say and no record has one.
	pub size: Option<u32>,
	/// Qdrant's distance; pgvector leaves it to the knowledge base.
	pub distance: Option<Distance>,
}

/// Result of inspecting a collection; a store failure is data for the page.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InspectResponse {
	pub error: Option<StoreError>,
	pub collection: Option<CollectionInfo>,
}

/// A knowledge base as the viewer sees it; `view` and `connection` only for knowledge admins.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeBaseResponse {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub mode: ConnectionMode,
	pub scope: KnowledgeScope,
	pub team_id: Option<Uuid>,
	pub team_name: Option<String>,
	/// The viewer owns this knowledge base (personal owner or admin of the owning team).
	pub owned: bool,
	/// Full-text search language for managed knowledge bases.
	pub search_language: String,
	/// Non-deleting document count for managed knowledge bases.
	pub document_count: u32,
	/// The viewer may change the name, description, search language and upload/edit managed documents.
	pub can_edit: bool,
	/// The viewer may change the source (external), share and delete it.
	pub can_manage: bool,
	/// Shared with or owned by the viewer, so chats may search it; knowledge admins also see bases that are not.
	pub usable: bool,
	pub connection: Option<KnowledgeBaseConnection>,
	pub view: Option<ExternalView>,
	/// Enabled models that may see the content; `None` without a restriction.
	#[cfg_attr(test, ts(type = "string[] | null"))]
	pub allowed_model_ids: Option<Vec<Uuid>>,
}

/// Team where the user may create a team-scoped managed knowledge base.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeScopeTeam {
	pub id: Uuid,
	pub name: String,
}

/// Where the caller may create new knowledge bases (`GET /api/v1/knowledge-bases/scopes`).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeScopesResponse {
	/// Default managed connection exists and has an embedding model configured.
	pub managed_available: bool,
	pub personal: bool,
	pub teams: Vec<KnowledgeScopeTeam>,
	pub installation: bool,
	pub external: bool,
}

/// One uploaded document inside a managed knowledge base.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RagDocumentResponse {
	pub id: Uuid,
	pub knowledge_base_id: Uuid,
	pub filename: String,
	pub mime_type: String,
	pub size_bytes: i64,
	pub status: DocumentStatus,
	/// Stable error reason code translated as `rag.doc_error.<error>`.
	pub error: Option<String>,
	pub page_count: Option<i32>,
	pub chunk_count: i32,
	pub token_count: i32,
	/// True when per-document "Limit access" is active (`access IS NOT NULL`).
	pub access_restricted: bool,
	/// True when a failed document still has text or source bytes to retry from.
	pub can_retry: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// The connection a knowledge base reads, for knowledge admins.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeBaseConnection {
	pub id: Uuid,
	pub name: String,
	pub kind: ConnectionKind,
}

/// Result of a test search; a store failure is data for the page.
#[derive(Debug, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeSearchResponse {
	pub error: Option<StoreError>,
	pub hits: Vec<KnowledgeHit>,
	/// Only when searching as another user: why they get these hits.
	pub access: Option<SearchAccess>,
}

/// What a user may find in a knowledge base, for "Search as".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SearchAccess {
	/// The knowledge base is shared with them.
	pub usable: bool,
	/// Records list who may see them.
	pub per_record: bool,
	/// Labels they hold.
	pub held: u32,
	/// Labels found in the collection or mapped.
	pub labels: u32,
	/// Labels found in the collection that nobody holds; `None` when the collection could not be read.
	pub unassigned: Option<u32>,
}

/// One access label of a knowledge base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AccessLabel {
	pub label: String,
	/// Found in the collection; mapped labels may not be.
	pub found: bool,
	/// Teams, roles and users holding it.
	pub holders: u32,
}

/// The access labels of a knowledge base and who holds them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AccessLabelsResponse {
	/// The payload field holding the labels; `None` without per-record access.
	pub field: Option<String>,
	pub labels: Vec<AccessLabel>,
	/// Labels found in the collection that nobody holds; their records are found by nobody.
	pub unassigned: u32,
	/// Why the collection's labels could not be read.
	pub error: Option<StoreError>,
}
