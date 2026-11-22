use super::{ConnectionKind, ConnectionMode, RagDefaults, RerankerSettings, StoreError};
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
