use super::{
	ChunkConfig, ChunkingSettings, ConnectionEmbedding, ConnectionKind, ConnectionMode, ConnectionResponse, ENV_PGVECTOR_URL, EmbeddingSettings, ModelTargets, RagConfig,
	RagConnection, RagDefaults, RagError, RagSetupResponse, ReindexEstimateResponse, ReindexProgress, RerankerSettings, SaveConnectionRequest, UpdateRagSetupRequest,
	UploadQuotaSettings, check_embedding_allowed, estimate_embedding_cost,
};
use crate::logging::AuditLog;
use crate::types::AuditEntry;
use crate::utils::encryption::encrypt_api_key;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

const AUDIT_CONNECTION: &str = "rag_connection";

/// Address, secret, embedding model, dimensions and vector size of a connection, read under a row lock.
type LockedRow = (Option<String>, Option<String>, Option<Uuid>, Option<i32>, Option<i32>);
const AUDIT_CONFIG: &str = "rag_config";

fn protect(secret: Option<&str>) -> Result<Option<String>, RagError> {
	secret
		.filter(|secret| !secret.is_empty())
		.map(|secret| encrypt_api_key(secret).map_err(|error| RagError::Database(sqlx::Error::Protocol(error.to_string()))))
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
	/// Whether knowledge is on: the Knowledge Search tool row exists and is enabled.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn enabled(pool: &PgPool) -> Result<bool, sqlx::Error> {
		crate::types::Tool::builtin_enabled(pool, super::KNOWLEDGE_BUILTIN).await
	}

	/// The installation's knowledge settings.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn get(pool: &PgPool) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			"SELECT reranker_model_id, reranker_top_n, default_connection_id, chunk_size, chunk_overlap_percent, max_uploads_per_hour, storage_quota_mb FROM rag_config",
		)
		.fetch_one(pool)
		.await
	}

	/// Chunking parameters for the ingest worker.
	#[must_use]
	pub fn chunk_config(&self) -> ChunkConfig {
		ChunkConfig {
			headings: true,
			target_tokens: usize::try_from(self.chunk_size).unwrap_or(super::DEFAULT_CHUNK_TOKENS),
			overlap_percent: usize::try_from(self.chunk_overlap_percent).unwrap_or(super::DEFAULT_OVERLAP_PERCENT),
		}
	}

	/// Saves the settings after checking the default connection is managed; the reranker must be checked by the caller.
	///
	/// # Errors
	///
	/// Returns `InvalidTopN`, `InvalidChunking`, `InvalidQuotas`, `DefaultNotManaged`, `NotFound` for an unknown connection, or a database error.
	pub async fn save(pool: &PgPool, actor_id: &Uuid, request: &UpdateRagSetupRequest) -> Result<(), RagError> {
		request.validate()?;
		if let Some(id) = &request.defaults.connection_id {
			let connection = RagConnection::find(pool, id).await?.ok_or(RagError::NotFound)?;
			if connection.mode != ConnectionMode::Managed {
				return Err(RagError::DefaultNotManaged);
			}
		}
		let current = Self::get(pool).await?;
		let chunking = request.chunking.unwrap_or(ChunkingSettings {
			chunk_size: u32::try_from(current.chunk_size).unwrap_or(400),
			overlap_percent: u32::try_from(current.chunk_overlap_percent).unwrap_or(15),
		});
		let quotas = request.quotas.unwrap_or(UploadQuotaSettings {
			max_uploads_per_hour: u32::try_from(current.max_uploads_per_hour).unwrap_or(60),
			storage_quota_mb: current.storage_quota_mb.and_then(|value| u32::try_from(value).ok()),
		});
		let mut tx = pool.begin().await?;
		sqlx::query(
			"UPDATE rag_config SET reranker_model_id = $1, reranker_top_n = $2, default_connection_id = $3,
			 chunk_size = $4, chunk_overlap_percent = $5, max_uploads_per_hour = $6, storage_quota_mb = $7, updated_at = NOW()",
		)
		.bind(request.reranker.model_id)
		.bind(to_i32(Some(request.reranker.top_n)))
		.bind(request.defaults.connection_id)
		.bind(to_i32(Some(chunking.chunk_size)))
		.bind(to_i32(Some(chunking.overlap_percent)))
		.bind(to_i32(Some(quotas.max_uploads_per_hour)))
		.bind(to_i32(quotas.storage_quota_mb))
		.execute(&mut *tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::NotFound))?;
		let metadata = serde_json::json!({ "reranker": request.reranker, "defaults": request.defaults, "chunking": chunking, "quotas": quotas });
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
		let progress = RagConnection::reindex_progress_map(pool).await?;
		Ok(RagSetupResponse {
			connections: connections
				.iter()
				.map(|connection| {
					let state = progress.get(&connection.id).copied();
					connection.to_response_with_progress(env && config.default_connection_id == Some(connection.id), state)
				})
				.collect(),
			reranker: RerankerSettings {
				model_id: config.reranker_model_id,
				top_n: u32::try_from(config.reranker_top_n).unwrap_or(1),
			},
			defaults: RagDefaults {
				connection_id: config.default_connection_id,
			},
			chunking: ChunkingSettings {
				chunk_size: u32::try_from(config.chunk_size).unwrap_or(400),
				overlap_percent: u32::try_from(config.chunk_overlap_percent).unwrap_or(15),
			},
			quotas: UploadQuotaSettings {
				max_uploads_per_hour: u32::try_from(config.max_uploads_per_hour).unwrap_or(60),
				storage_quota_mb: config.storage_quota_mb.and_then(|value| u32::try_from(value).ok()),
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
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::ModelUnavailable))?;
		connection.make_default_if_none(&mut tx).await?;
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
	/// Returns `NotFound` when the connection is gone, `SecretRequiredForNewHost` when the stored secret would follow a
	/// changed address, or a database error.
	pub async fn update(&self, pool: &PgPool, actor_id: &Uuid, request: &SaveConnectionRequest, vector_size: Option<u32>) -> Result<Self, RagError> {
		let new_secret = protect(request.secret.as_deref())?;
		let mut tx = pool.begin().await?;
		let current = self.locked(&mut tx).await?;
		if current.redirects_secret(request) {
			return Err(RagError::SecretRequiredForNewHost);
		}
		let (vector_size, reindex) = current.next_index(request, vector_size);
		current.check_managed_model_conflicts(&mut tx, request.embedding.as_ref()).await?;
		let connection = self.apply_update(&mut tx, request, new_secret, (vector_size, reindex)).await?;
		connection.make_default_if_none(&mut tx).await?;
		if reindex {
			connection.queue_reindex(&mut tx, actor_id).await?;
		}
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
		super::forget_pool(&connection.id);
		Ok(connection)
	}

	async fn make_default_if_none(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), sqlx::Error> {
		if self.mode == ConnectionMode::Managed {
			sqlx::query("UPDATE rag_config SET default_connection_id = $1 WHERE default_connection_id IS NULL")
				.bind(self.id)
				.execute(&mut **tx)
				.await?;
		}
		Ok(())
	}

	async fn apply_update(
		&self,
		tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
		request: &SaveConnectionRequest,
		new_secret: Option<String>,
		(vector_size, reindex): (Option<i32>, bool),
	) -> Result<Self, RagError> {
		let clear_secret = request.secret.as_deref() == Some("");
		let embedding = request.embedding.as_ref();
		sqlx::query_as!(
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
		.fetch_optional(&mut **tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::ModelUnavailable))?
		.ok_or(RagError::NotFound)
	}

	async fn check_managed_model_conflicts(
		&self,
		tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
		embedding: Option<&EmbeddingSettings>,
	) -> Result<(), RagError> {
		let Some(embedding) = embedding.filter(|_| self.mode == ConnectionMode::Managed) else {
			return Ok(());
		};
		let conflicts = Self::conflicting_knowledge_bases(tx, &self.id, &embedding.model_id).await?;
		if !conflicts.is_empty() {
			return Err(RagError::ConnectionModelConflict(conflicts));
		}
		Ok(())
	}

	async fn queue_reindex(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, actor_id: &Uuid) -> Result<(), sqlx::Error> {
		if self.mode != ConnectionMode::Managed || self.embedding_model_id.is_none() {
			return Ok(());
		}
		sqlx::query(
			"UPDATE rag_documents d SET status = 'pending', reindex_by = $2, error = NULL, locked_at = NULL, updated_at = NOW()
			 FROM knowledge_bases kb
			 WHERE d.knowledge_base_id = kb.id AND kb.connection_id = $1 AND kb.deleted_at IS NULL AND d.status <> 'deleting'",
		)
		.bind(self.id)
		.bind(actor_id)
		.execute(&mut **tx)
		.await?;
		Ok(())
	}

	fn next_index(&self, request: &SaveConnectionRequest, vector_size: Option<u32>) -> (Option<i32>, bool) {
		let vector_size = match (&request.embedding, to_i32(vector_size)) {
			(_, Some(size)) => Some(size),
			(None, None) => None,
			(Some(_), None) => self.vector_size,
		};
		let reindex = vector_size != self.vector_size || (request.embedding.is_some() && request.embedding != self.embedding_settings());
		(vector_size, reindex)
	}

	async fn locked(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<Self, RagError> {
		let row: Option<LockedRow> =
			sqlx::query_as("SELECT url, secret, embedding_model_id, dimensions, vector_size FROM rag_connections WHERE id = $1 FOR UPDATE")
				.bind(self.id)
				.fetch_optional(&mut **tx)
				.await?;
		let (url, secret, embedding_model_id, dimensions, vector_size) = row.ok_or(RagError::NotFound)?;
		Ok(Self {
			url,
			secret,
			embedding_model_id,
			dimensions,
			vector_size,
			..self.clone()
		})
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
		super::forget_pool(id);
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

	/// Knowledge bases on `connection_id` whose model restrictions do not allow `embedding_model_id`.
	///
	/// # Errors
	///
	/// Returns `ModelUnavailable` or a database error.
	pub async fn conflicting_knowledge_bases(conn: &mut sqlx::PgConnection, connection_id: &Uuid, embedding_model_id: &Uuid) -> Result<Vec<String>, RagError> {
		let bases: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM knowledge_bases WHERE connection_id = $1 AND deleted_at IS NULL ORDER BY lower(name), id")
			.bind(connection_id)
			.fetch_all(&mut *conn)
			.await?;
		if bases.is_empty() {
			return Ok(Vec::new());
		}
		let ids: Vec<Uuid> = bases.iter().map(|(id, _)| *id).collect();
		let targets = ModelTargets::load(&mut *conn, &ids).await?;
		let mut conflicts = Vec::new();
		for (id, name) in bases {
			if let Some(target) = targets.get(&id)
				&& matches!(check_embedding_allowed(&mut *conn, target, embedding_model_id).await, Err(RagError::EmbeddingModelNotAllowed))
			{
				conflicts.push(name);
			}
		}
		Ok(conflicts)
	}

	/// Whether any document on `connection_id` still waits to be re-indexed to `index_generation`; failed documents do not count.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn is_reindexing(&self, pool: &PgPool) -> Result<bool, sqlx::Error> {
		if self.mode != ConnectionMode::Managed {
			return Ok(false);
		}
		sqlx::query_scalar(
			"SELECT EXISTS (
				SELECT 1 FROM rag_documents d JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id
				WHERE kb.connection_id = $1 AND kb.deleted_at IS NULL AND d.status IN ('pending', 'processing') AND d.index_generation < $2
			)",
		)
		.bind(self.id)
		.bind(self.index_generation)
		.fetch_one(pool)
		.await
	}

	/// Per-connection re-indexing progress for connections that still have documents behind their current `index_generation`.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn reindex_progress_map(pool: &PgPool) -> Result<HashMap<Uuid, ReindexProgress>, sqlx::Error> {
		let rows: Vec<(Uuid, i64, i64)> = sqlx::query_as(
			"SELECT c.id,
			        COUNT(*) FILTER (WHERE d.index_generation >= c.index_generation AND d.status = 'ready')::bigint,
			        COUNT(*)::bigint
			 FROM rag_connections c
			 JOIN knowledge_bases kb ON kb.connection_id = c.id AND kb.deleted_at IS NULL
			 JOIN rag_documents d ON d.knowledge_base_id = kb.id AND d.status <> 'deleting'
			 WHERE c.mode = 'managed'
			 GROUP BY c.id
			 HAVING COUNT(*) FILTER (WHERE d.index_generation < c.index_generation AND d.status IN ('pending', 'processing')) > 0",
		)
		.fetch_all(pool)
		.await?;
		Ok(rows
			.into_iter()
			.map(|(id, done, total)| {
				(
					id,
					ReindexProgress {
						done: u32::try_from(done).unwrap_or(u32::MAX),
						total: u32::try_from(total).unwrap_or(u32::MAX),
					},
				)
			})
			.collect())
	}

	/// Chunk count, estimated cost and conflicting knowledge bases for re-embedding this managed connection with `embedding`.
	///
	/// # Errors
	///
	/// Returns `DefaultNotManaged`, `ModelUnavailable` or a database error.
	pub async fn reindex_estimate(&self, pool: &PgPool, embedding: &EmbeddingSettings) -> Result<ReindexEstimateResponse, RagError> {
		if self.mode != ConnectionMode::Managed {
			return Err(RagError::DefaultNotManaged);
		}
		let (document_count, chunk_count, total_tokens): (i64, i64, i64) = sqlx::query_as(
			"SELECT COUNT(DISTINCT d.id)::bigint, COUNT(ch.id)::bigint, COALESCE(SUM(ch.token_count), 0)::bigint
			 FROM knowledge_bases kb
			 JOIN rag_documents d ON d.knowledge_base_id = kb.id AND d.status <> 'deleting'
			 LEFT JOIN rag_chunks ch ON ch.document_id = d.id
			 WHERE kb.connection_id = $1 AND kb.deleted_at IS NULL",
		)
		.bind(self.id)
		.fetch_one(pool)
		.await?;
		let mut conn = pool.acquire().await?;
		let conflicting_knowledge_bases = Self::conflicting_knowledge_bases(&mut conn, &self.id, &embedding.model_id).await?;
		let tokens = u64::try_from(total_tokens).unwrap_or(0);
		let estimated_cost_usd = estimate_embedding_cost(pool, &embedding.model_id, tokens).await?;
		Ok(ReindexEstimateResponse {
			chunk_count: u64::try_from(chunk_count).unwrap_or(0),
			document_count: u64::try_from(document_count).unwrap_or(0),
			estimated_tokens: tokens,
			estimated_cost_usd,
			conflicting_knowledge_bases,
		})
	}

	/// The admin view; `url_from_env` says `RAG_PGVECTOR_URL` replaces the address.
	#[cfg(test)]
	#[must_use]
	pub fn to_response(&self, url_from_env: bool) -> ConnectionResponse {
		self.to_response_with_progress(url_from_env, None)
	}

	/// The admin view with optional re-indexing progress.
	#[must_use]
	pub fn to_response_with_progress(&self, url_from_env: bool, reindex_progress: Option<ReindexProgress>) -> ConnectionResponse {
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
			reindexing: reindex_progress.is_some(),
			reindex_progress,
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

/// Whether two connection addresses share scheme, host, port and `sslmode`; `None` is the main database.
#[must_use]
fn same_host(first: Option<&str>, second: Option<&str>) -> bool {
	let parts = |url: Option<&str>| {
		url.map(|url| {
			reqwest::Url::parse(url).ok().map(|url| {
				let sslmode = url.query_pairs().find(|(key, _)| key == "sslmode").map(|(_, value)| value.into_owned());
				(url.scheme().to_owned(), url.host_str().map(str::to_owned), url.port_or_known_default(), sslmode)
			})
		})
	};
	match (parts(first), parts(second)) {
		(None, None) => true,
		(Some(Some(first)), Some(Some(second))) => first == second,
		_ => false,
	}
}
