use super::{ConnectionKind, ConnectionMode, Distance, ExternalView, KnowledgeHit, RagDefaults, RerankerSettings, StoreError};
use serde::Serialize;
use uuid::Uuid;

/// Everything the knowledge setup page shows.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RagSetupResponse {
	pub enabled: bool,
	pub connections: Vec<ConnectionResponse>,
	pub reranker: RerankerSettings,
	pub defaults: RagDefaults,
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
	/// The viewer may change the name and description.
	pub can_edit: bool,
	/// The viewer may change the source, share and delete it.
	pub can_manage: bool,
	/// Shared with the viewer, so chats may search it; knowledge admins also see bases that are not.
	pub usable: bool,
	pub connection: Option<KnowledgeBaseConnection>,
	pub view: Option<ExternalView>,
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
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeSearchResponse {
	pub error: Option<StoreError>,
	pub hits: Vec<KnowledgeHit>,
}
