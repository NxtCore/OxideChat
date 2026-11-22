//! Knowledge (RAG): connections to vector stores and the models that index and rerank knowledge.

mod knowledge_bases;
mod models;
mod repository;
mod requests;
mod responses;
mod store;
mod view;

pub use knowledge_bases::*;
pub use models::*;
pub use requests::*;
pub use responses::*;
pub use store::*;
pub use view::*;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Overrides the address and password of the default managed connection.
pub const ENV_PGVECTOR_URL: &str = "RAG_PGVECTOR_URL";

/// Which vector store a connection talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ConnectionKind {
	Pgvector,
	Qdrant,
}

/// Whether `OxideChat` writes to a connection or only searches a collection another pipeline fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ConnectionMode {
	Managed,
	ReadOnly,
}

/// A stored connection; `secret` is encrypted.
#[derive(Debug, Clone)]
pub struct RagConnection {
	pub id: Uuid,
	pub name: String,
	pub kind: ConnectionKind,
	pub mode: ConnectionMode,
	pub url: Option<String>,
	pub secret: Option<String>,
	pub embedding_model_id: Option<Uuid>,
	pub dimensions: Option<i32>,
	pub vector_size: Option<i32>,
	pub index_generation: i32,
}

/// The installation's knowledge settings (single row).
#[derive(Debug, Clone)]
pub struct RagConfig {
	pub enabled: bool,
	pub reranker_model_id: Option<Uuid>,
	pub reranker_top_n: i32,
	pub default_connection_id: Option<Uuid>,
}

/// Why knowledge settings or a connection could not be saved; `reason()` is translated as `rag.error.<reason>`.
#[derive(Debug, thiserror::Error)]
pub enum RagError {
	#[error("invalid name")]
	InvalidName,
	#[error("invalid url")]
	InvalidUrl,
	#[error("the url contains a password")]
	UrlHasPassword,
	#[error("the connection needs a url")]
	UrlRequired,
	#[error("managed connections must use pgvector")]
	ManagedNeedsPgvector,
	#[error("API keys are only sent over https")]
	InsecureSecret,
	#[error("read-only connections have no embedding model")]
	ReadOnlyHasModel,
	#[error("invalid dimensions")]
	InvalidDimensions,
	#[error("not an embedding model")]
	NotEmbeddingModel,
	#[error("not a reranking model")]
	NotRerankingModel,
	#[error("the model or its provider is missing or disabled")]
	ModelUnavailable,
	#[error("the test embedding failed: {0}")]
	EmbeddingFailed(String),
	#[error("the model returned vectors of another size")]
	DimensionsMismatch,
	#[error("the budget is used up")]
	BudgetExceeded,
	#[error("the default connection must be managed")]
	DefaultNotManaged,
	#[error("invalid top_n")]
	InvalidTopN,
	#[error("invalid collection")]
	InvalidCollection,
	#[error("invalid field")]
	InvalidField,
	#[error("invalid filter")]
	InvalidFilter,
	#[error("knowledge bases of this kind need a read-only connection")]
	NotReadOnly,
	#[error("the collection has no vector of that name")]
	VectorNotFound,
	#[error("the model's vectors do not have the collection's size")]
	CollectionDimensionsMismatch,
	#[error("the vector store could not be read: {0}")]
	Store(#[from] StoreError),
	#[error("knowledge bases still use the connection")]
	ConnectionInUse,
	#[error("invalid assignment")]
	InvalidAssignment,
	#[error("invalid query")]
	InvalidQuery,
	#[error("not allowed")]
	Forbidden,
	#[error("connection not found")]
	NotFound,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

impl RagError {
	/// `instead` when the database refused `error` for a foreign key, otherwise the database error.
	#[must_use]
	pub fn foreign_key_or(error: sqlx::Error, instead: Self) -> Self {
		if error.as_database_error().is_some_and(sqlx::error::DatabaseError::is_foreign_key_violation) {
			instead
		} else {
			Self::Database(error)
		}
	}

	/// The translation suffix shown to admins.
	#[must_use]
	pub const fn reason(&self) -> &'static str {
		match self {
			Self::InvalidName => "invalid_name",
			Self::InvalidUrl => "invalid_url",
			Self::UrlHasPassword => "url_has_password",
			Self::UrlRequired => "url_required",
			Self::ManagedNeedsPgvector => "managed_needs_pgvector",
			Self::InsecureSecret => "insecure_secret",
			Self::ReadOnlyHasModel => "read_only_has_model",
			Self::InvalidDimensions => "invalid_dimensions",
			Self::NotEmbeddingModel => "not_embedding_model",
			Self::NotRerankingModel => "not_reranking_model",
			Self::ModelUnavailable => "model_unavailable",
			Self::EmbeddingFailed(_) => "embedding_failed",
			Self::DimensionsMismatch => "dimensions_mismatch",
			Self::BudgetExceeded => "budget_exceeded",
			Self::DefaultNotManaged => "default_not_managed",
			Self::InvalidTopN => "invalid_top_n",
			Self::InvalidCollection => "invalid_collection",
			Self::InvalidField => "invalid_field",
			Self::InvalidFilter => "invalid_filter",
			Self::NotReadOnly => "not_read_only",
			Self::VectorNotFound => "vector_not_found",
			Self::CollectionDimensionsMismatch => "collection_dimensions_mismatch",
			Self::Store(_) => "store_failed",
			Self::ConnectionInUse => "connection_in_use",
			Self::InvalidAssignment => "invalid_assignment",
			Self::InvalidQuery => "invalid_query",
			Self::Forbidden => "forbidden",
			Self::NotFound => "not_found",
			Self::Database(_) => "database",
		}
	}
}
