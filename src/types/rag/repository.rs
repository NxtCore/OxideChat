use super::{
	ConnectionEmbedding, ConnectionKind, ConnectionMode, ConnectionResponse, ENV_PGVECTOR_URL, EmbeddingSettings, RagConfig, RagConnection, RagDefaults, RagError,
	RagSetupResponse, RerankerSettings, SaveConnectionRequest, UpdateRagSetupRequest,
};
use crate::logging::AuditLog;
use crate::types::AuditEntry;
use crate::utils::encryption::encrypt_secret_value;
use sqlx::PgPool;
use uuid::Uuid;

const AUDIT_CONNECTION: &str = "rag_connection";
const AUDIT_CONFIG: &str = "rag_config";

fn protect(secret: Option<&str>) -> Result<Option<String>, RagError> {
	secret
		.filter(|secret| !secret.is_empty())
		.map(|secret| encrypt_secret_value(secret, None).map_err(|error| RagError::Database(sqlx::Error::Protocol(error.to_string()))))
		.transpose()
}

fn to_i32(value: Option<u32>) -> Option<i32> {
	value.and_then(|value| i32::try_from(value).ok())
}

/// `RAG_PGVECTOR_URL`, when set and not empty.
#[must_use]
fn env_pgvector_url() -> Option<String> {
	std::env::var(ENV_PGVECTOR_URL).ok().filter(|url| !url.trim().is_empty())
}

impl RagConfig {
	/// The installation's knowledge settings.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn get(pool: &PgPool) -> Result<Self, sqlx::Error> {
		sqlx::query_as!(Self, "SELECT enabled, reranker_model_id, reranker_top_n, default_connection_id FROM rag_config")
			.fetch_one(pool)
			.await
	}

	/// Saves the settings after checking the default connection is managed; the reranker must be checked by the caller.
	///
	/// # Errors
	///
	/// Returns `InvalidTopN`, `DefaultNotManaged`, `NotFound` for an unknown connection, or a database error.
	pub async fn save(pool: &PgPool, actor_id: &Uuid, request: &UpdateRagSetupRequest) -> Result<(), RagError> {
		request.validate()?;
		if let Some(id) = &request.defaults.connection_id {
			let connection = RagConnection::find(pool, id).await?.ok_or(RagError::NotFound)?;
			if connection.mode != ConnectionMode::Managed {
				return Err(RagError::DefaultNotManaged);
			}
		}
		let mut tx = pool.begin().await?;
		sqlx::query!(
			"UPDATE rag_config SET enabled = $1, reranker_model_id = $2, reranker_top_n = $3, default_connection_id = $4, updated_at = NOW()",
			request.enabled,
			request.reranker.model_id,
			to_i32(Some(request.reranker.top_n)),
			request.defaults.connection_id,
		)
		.execute(&mut *tx)
		.await?;
		let metadata = serde_json::json!({ "enabled": request.enabled, "reranker": request.reranker, "defaults": request.defaults });
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_config_updated",
				actor_id,
				target_type: AUDIT_CONFIG,
				target_id: &Uuid::nil(),
				metadata,
			},
		)
		.await?;
		tx.commit().await?;
		Ok(())
	}

	/// The setup page: settings and every connection.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn setup(pool: &PgPool) -> Result<RagSetupResponse, sqlx::Error> {
		let config = Self::get(pool).await?;
		let env = env_pgvector_url().is_some();
		let connections = RagConnection::list(pool).await?;
		Ok(RagSetupResponse {
			enabled: config.enabled,
			connections: connections
				.iter()
				.map(|connection| connection.to_response(env && config.default_connection_id == Some(connection.id)))
				.collect(),
			reranker: RerankerSettings {
				model_id: config.reranker_model_id,
				top_n: u32::try_from(config.reranker_top_n).unwrap_or(1),
			},
			defaults: RagDefaults {
				connection_id: config.default_connection_id,
			},
		})
	}

	/// The address and password `RAG_PGVECTOR_URL` gives `connection`, if it is the default managed connection.
	#[must_use]
	pub fn env_url_for(&self, connection: &RagConnection) -> Option<String> {
		(self.default_connection_id == Some(connection.id) && connection.mode == ConnectionMode::Managed)
			.then(env_pgvector_url)
			.flatten()
	}
}

impl RagConnection {
	/// Every connection, by name.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as!(
			Self,
			r#"SELECT id, name, kind AS "kind: ConnectionKind", mode AS "mode: ConnectionMode", url, secret, embedding_model_id, dimensions, vector_size, index_generation
			   FROM rag_connections ORDER BY name"#
		)
		.fetch_all(pool)
		.await
	}

	/// One connection.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as!(
			Self,
			r#"SELECT id, name, kind AS "kind: ConnectionKind", mode AS "mode: ConnectionMode", url, secret, embedding_model_id, dimensions, vector_size, index_generation
			   FROM rag_connections WHERE id = $1"#,
			id
		)
		.fetch_optional(pool)
		.await
	}

	/// Adds a connection; `vector_size` is what the test embedding of `request.embedding` returned.
	///
	/// # Errors
	///
	/// Returns a database error, for example on a duplicate name.
	pub async fn create(pool: &PgPool, actor_id: &Uuid, request: &SaveConnectionRequest, vector_size: Option<u32>) -> Result<Self, RagError> {
		let secret = protect(request.secret.as_deref())?;
		let embedding = request.embedding.as_ref();
		let mut tx = pool.begin().await?;
		let connection = sqlx::query_as!(
			Self,
			r#"INSERT INTO rag_connections (name, kind, mode, url, secret, embedding_model_id, dimensions, vector_size)
			   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
			   RETURNING id, name, kind AS "kind: ConnectionKind", mode AS "mode: ConnectionMode", url, secret, embedding_model_id, dimensions, vector_size, index_generation"#,
			request.name.trim(),
			request.kind as ConnectionKind,
			request.mode as ConnectionMode,
			request.url(),
			secret,
			embedding.map(|embedding| embedding.model_id),
			to_i32(embedding.and_then(|embedding| embedding.dimensions)),
			to_i32(vector_size),
		)
		.fetch_one(&mut *tx)
		.await?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_connection_created",
				actor_id,
				target_type: AUDIT_CONNECTION,
				target_id: &connection.id,
				metadata: connection.audit_metadata(),
			},
		)
		.await?;
		tx.commit().await?;
		Ok(connection)
	}

	/// Changes name, address, secret and embedding model; `kind` and `mode` stay. A new model or size bumps `index_generation`.
	///
	/// `vector_size` is the new test embedding's size, or `None` to keep the stored one.
	///
	/// # Errors
	///
	/// Returns `NotFound` when the connection is gone, or a database error.
	pub async fn update(&self, pool: &PgPool, actor_id: &Uuid, request: &SaveConnectionRequest, vector_size: Option<u32>) -> Result<Self, RagError> {
		let new_secret = protect(request.secret.as_deref())?;
		let clear_secret = request.secret.as_deref() == Some("");
		let embedding = request.embedding.as_ref();
		let vector_size = to_i32(vector_size).or(self.vector_size);
		let reindex = vector_size != self.vector_size || (request.embedding.is_some() && request.embedding != self.embedding_settings());
		let mut tx = pool.begin().await?;
		let connection = sqlx::query_as!(
			Self,
			r#"UPDATE rag_connections SET
			   name = $2, url = $3,
			   secret = CASE WHEN $4 THEN NULL ELSE COALESCE($5, secret) END,
			   embedding_model_id = $6, dimensions = $7, vector_size = $8,
			   index_generation = index_generation + CASE WHEN $9 THEN 1 ELSE 0 END,
			   updated_at = NOW()
			   WHERE id = $1
			   RETURNING id, name, kind AS "kind: ConnectionKind", mode AS "mode: ConnectionMode", url, secret, embedding_model_id, dimensions, vector_size, index_generation"#,
			self.id,
			request.name.trim(),
			request.url(),
			clear_secret,
			new_secret,
			embedding.map(|embedding| embedding.model_id),
			to_i32(embedding.and_then(|embedding| embedding.dimensions)),
			vector_size,
			reindex,
		)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(RagError::NotFound)?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_connection_updated",
				actor_id,
				target_type: AUDIT_CONNECTION,
				target_id: &connection.id,
				metadata: connection.audit_metadata(),
			},
		)
		.await?;
		tx.commit().await?;
		Ok(connection)
	}

	/// Removes a connection; returns whether it existed.
	///
	/// # Errors
	///
	/// Returns `ConnectionInUse` while knowledge bases point at it, or a database error.
	pub async fn delete(pool: &PgPool, actor_id: &Uuid, id: &Uuid) -> Result<bool, RagError> {
		let mut tx = pool.begin().await?;
		let deleted = sqlx::query_scalar!("DELETE FROM rag_connections WHERE id = $1 RETURNING name", id)
			.fetch_optional(&mut *tx)
			.await
			.map_err(|error| RagError::foreign_key_or(error, RagError::ConnectionInUse))?;
		let Some(name) = deleted else {
			return Ok(false);
		};
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_connection_deleted",
				actor_id,
				target_type: AUDIT_CONNECTION,
				target_id: id,
				metadata: serde_json::json!({ "name": name }),
			},
		)
		.await?;
		tx.commit().await?;
		Ok(true)
	}

	/// The embedding model and requested size as the setup page sends them.
	#[must_use]
	pub fn embedding_settings(&self) -> Option<EmbeddingSettings> {
		self.embedding_model_id.map(|model_id| EmbeddingSettings {
			model_id,
			dimensions: self.dimensions.and_then(|dimensions| u32::try_from(dimensions).ok()),
		})
	}

	/// Whether saving `request` would send the stored secret to another host.
	#[must_use]
	pub fn redirects_secret(&self, request: &SaveConnectionRequest) -> bool {
		let keeps_secret = self.secret.is_some() && request.secret.is_none();
		keeps_secret && !same_host(self.url.as_deref(), request.url())
	}

	/// The admin view; `url_from_env` says `RAG_PGVECTOR_URL` replaces the address.
	#[must_use]
	pub fn to_response(&self, url_from_env: bool) -> ConnectionResponse {
		ConnectionResponse {
			id: self.id,
			name: self.name.clone(),
			kind: self.kind,
			mode: self.mode,
			url: self.url.clone(),
			has_secret: self.secret.is_some(),
			url_from_env,
			embedding: self.vector_size.and_then(|size| u32::try_from(size).ok()).map(|vector_size| ConnectionEmbedding {
				model_id: self.embedding_model_id,
				dimensions: self.embedding_settings().and_then(|settings| settings.dimensions),
				vector_size,
			}),
		}
	}

	fn audit_metadata(&self) -> serde_json::Value {
		serde_json::json!({
			"name": self.name,
			"kind": self.kind,
			"mode": self.mode,
			"host": self.url.as_deref().and_then(|url| reqwest::Url::parse(url).ok()).and_then(|url| url.host_str().map(str::to_owned)),
			"embedding_model_id": self.embedding_model_id,
			"dimensions": self.dimensions,
			"vector_size": self.vector_size,
			"index_generation": self.index_generation,
		})
	}
}

/// Whether two connection addresses share scheme, host and port; `None` is the main database.
#[must_use]
fn same_host(first: Option<&str>, second: Option<&str>) -> bool {
	let parts = |url: Option<&str>| {
		url.map(|url| {
			reqwest::Url::parse(url)
				.ok()
				.map(|url| (url.scheme().to_owned(), url.host_str().map(str::to_owned), url.port_or_known_default()))
		})
	};
	match (parts(first), parts(second)) {
		(None, None) => true,
		(Some(Some(first)), Some(Some(second))) => first == second,
		_ => false,
	}
}
