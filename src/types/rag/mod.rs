//! Knowledge (RAG): connections to vector stores and the models that index and rerank knowledge.

mod models;
mod repository;
mod requests;
mod responses;
mod store;

pub use models::*;
pub use requests::*;
pub use responses::*;
pub use store::*;

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
	#[error("connection not found")]
	NotFound,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

impl RagError {
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
			Self::NotFound => "not_found",
			Self::Database(_) => "database",
		}
	}
}
