//! Read access to the vector stores behind connections.

use super::view::{field_paths, pg_access, pg_access_value, pg_filter, qdrant_access, qdrant_filter, quote_ident, shorten};
use super::{CollectionInfo, ConnectionKind, ConnectionMode, Distance, ExternalView, FieldFilter, RagConnection, VectorInfo};
use crate::utils::encryption::decrypt_api_key;
use crate::utils::outbound::{guarded_client, is_allowed_ip, private_hosts_allowed, read_limited};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::types::Oid;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, PgPool};
use arc_swap::ArcSwap;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_COLLECTIONS: i64 = 500;
const MIN_PGVECTOR: (u32, u32) = (0, 8);
const SAMPLE_SIZE: i64 = 3;
const MAX_SEARCH_HITS: u32 = 50;
const POOL_SIZE: u32 = 4;
const POOL_IDLE: Duration = Duration::from_secs(60);

/// Most distinct access labels discovery reads from a collection.
pub const MAX_ACCESS_LABELS: u32 = 500;

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
	#[error("the collection does not exist")]
	CollectionNotFound,
	#[error("the collection has no such field or vector")]
	UnknownField,
	#[error("the connection is re-indexing")]
	Reindexing,
}

/// One chunk vector written to a managed pgvector connection.
#[derive(Debug, Clone)]
pub struct ManagedVector {
	pub chunk_id: Uuid,
	pub knowledge_base_id: Uuid,
	pub document_id: Uuid,
	pub access: Option<Vec<String>>,
	pub embedding: Vec<f32>,
}

/// Parameters for a managed pgvector similarity search.
pub struct ManagedSearch<'a> {
	pub connection_id: &'a Uuid,
	pub generation: i32,
	pub knowledge_base_id: &'a Uuid,
	pub vector: &'a [f32],
	pub limit: u32,
	pub labels: &'a [String],
}

/// Deterministic table name for one managed connection and index generation (`rag_vectors_<hex16>_g<gen>`).
#[must_use]
pub fn managed_table_name(connection_id: &Uuid, generation: i32) -> String {
	let hex = connection_id.simple().to_string();
	let prefix = &hex[..16];
	let generation = generation.max(0);
	format!("rag_vectors_{prefix}_g{generation}")
}

fn managed_table_prefix(connection_id: &Uuid) -> String {
	let hex = connection_id.simple().to_string();
	format!("rag_vectors_{}_g", &hex[..16])
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

#[derive(Deserialize)]
struct QdrantResult<T> {
	result: T,
}

#[derive(Deserialize)]
struct QdrantPoints {
	points: Vec<QdrantPoint>,
}

#[derive(Deserialize)]
struct QdrantFacet {
	hits: Vec<QdrantFacetHit>,
}

#[derive(Deserialize)]
struct QdrantFacetHit {
	value: Value,
}

#[derive(Deserialize)]
struct QdrantPoint {
	#[serde(default)]
	score: f64,
	#[serde(default)]
	payload: Value,
}

/// A pgvector table: what to put after `FROM` and its columns as (name, type, type modifier).
struct PgTable {
	from: String,
	columns: Vec<(String, String, i32)>,
}

impl PgTable {
	fn vector_columns(&self) -> Vec<String> {
		self.columns
			.iter()
			.filter(|(_, kind, _)| is_pg_vector(kind))
			.map(|(name, _, _)| name.clone())
			.collect()
	}

	fn columns(&self) -> Vec<String> {
		self.columns.iter().map(|(name, _, _)| name.clone()).collect()
	}

	fn vector_type(&self, name: &str) -> Option<&str> {
		self.columns
			.iter()
			.find(|(column, kind, _)| column == name && is_pg_vector(kind))
			.map(|(_, kind, _)| kind.as_str())
	}
}

fn is_pg_vector(kind: &str) -> bool {
	matches!(kind, "vector" | "halfvec")
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
				(Some(url), _) => connect_postgres(connection, url, None, true).await,
				(None, Some(url)) => connect_postgres(connection, url, secret.as_deref(), false).await,
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

	/// The collection's vectors, its payload fields and a few records, filtered like `filter`.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be read, the collection is missing or a filter names an unknown column.
	pub async fn describe(&self, collection: &str, filter: &[FieldFilter]) -> Result<CollectionInfo, StoreError> {
		match self {
			Self::Pgvector(pool) => {
				let table = pg_table(pool, collection).await?;
				let (clause, params) = pg_filter(filter, &table.columns(), 2).ok_or(StoreError::UnknownField)?;
				let sql = format!("SELECT to_jsonb(t) - $1::text[] FROM {} t{clause} LIMIT {SAMPLE_SIZE}", table.from);
				let query = params.iter().fold(
					sqlx::query_scalar::<_, Value>(AssertSqlSafe(sql)).bind(table.vector_columns()),
					sqlx::query::QueryScalar::bind,
				);
				let sample = timed(query.fetch_all(pool.as_ref())).await?;
				let mut vectors = Vec::new();
				for (name, kind, modifier) in table.columns.iter().filter(|(_, kind, _)| is_pg_vector(kind)) {
					let size = match u32::try_from(*modifier) {
						Ok(size) if size > 0 => Some(size),
						_ => pg_vector_size(pool, &table, name, kind).await?,
					};
					vectors.push(VectorInfo {
						name: Some(name.clone()),
						size,
						distance: None,
					});
				}
				let mut fields: Vec<String> = table
					.columns
					.iter()
					.filter(|(_, kind, _)| !is_pg_vector(kind))
					.map(|(name, _, _)| name.clone())
					.collect();
				fields.extend(field_paths(&sample));
				fields.sort();
				fields.dedup();
				Ok(CollectionInfo {
					vectors,
					fields,
					sample: sample.into_iter().map(shorten).collect(),
				})
			}
			Self::Qdrant(qdrant) => {
				let info: QdrantResult<Value> = qdrant.send(reqwest::Method::GET, &["collections", collection], None).await?;
				let vectors = qdrant_vectors(info.result.pointer("/config/params/vectors").unwrap_or(&Value::Null));
				let mut body = serde_json::json!({ "limit": SAMPLE_SIZE, "with_payload": true, "with_vector": false });
				if let Some(filter) = qdrant_filter(filter, None) {
					body["filter"] = filter;
				}
				let points: QdrantResult<QdrantPoints> = qdrant
					.send(reqwest::Method::POST, &["collections", collection, "points", "scroll"], Some(&body))
					.await?;
				let sample: Vec<Value> = points.result.points.into_iter().map(|point| point.payload).collect();
				Ok(CollectionInfo {
					vectors,
					fields: field_paths(&sample),
					sample: sample.into_iter().map(shorten).collect(),
				})
			}
		}
	}

	/// The `limit` records of `view` nearest to `vector` as (payload, score), nearest first.
	///
	/// With per-record access (`view.access`) only records listing one of `labels` are searched, inside the query so the limit is not cut short;
	/// without labels nothing is.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be read, the collection is missing or the view names an unknown field.
	pub async fn search(&self, view: &ExternalView, vector: &[f32], limit: u32, labels: &[String]) -> Result<Vec<(Value, f64)>, StoreError> {
		let limit = limit.clamp(1, MAX_SEARCH_HITS);
		if view.access.is_some() && labels.is_empty() {
			return Ok(Vec::new());
		}
		match self {
			Self::Pgvector(pool) => {
				let table = pg_table(pool, &view.collection).await?;
				let column = view.vector_name.as_deref().ok_or(StoreError::UnknownField)?;
				let kind = table.vector_type(column).ok_or(StoreError::UnknownField)?;
				let (clause, params) = pg_filter(&view.filter, &table.columns(), 6).ok_or(StoreError::UnknownField)?;
				let not_null = format!("t.{} IS NOT NULL", quote_ident(column));
				let mut clause = if clause.is_empty() { format!(" WHERE {not_null}") } else { format!("{clause} AND {not_null}") };
				let mut path = Vec::new();
				if let Some(field) = &view.access {
					let (condition, segments) = pg_access(field, &table.columns, 4, 5).ok_or(StoreError::UnknownField)?;
					clause = format!("{clause} AND {condition}");
					path = segments;
				}
				let cast = if kind == "halfvec" { "::vector::halfvec" } else { "::vector" };
				let distance = format!("(t.{} {} $2::real[]{cast})", quote_ident(column), view.distance.pg_operator());
				let sql = format!(
					"SELECT to_jsonb(t) - $1::text[], {distance}::float8 FROM {} t{clause} ORDER BY {distance} LIMIT $3",
					table.from
				);
				let query = sqlx::query_as::<_, (Value, f64)>(AssertSqlSafe(sql))
					.bind(table.vector_columns())
					.bind(vector)
					.bind(i64::from(limit))
					.bind(labels)
					.bind(path);
				let query = params.iter().fold(query, sqlx::query::QueryAs::bind);
				let mut tx = timed(pool.begin()).await?;
				timed(sqlx::query("SET LOCAL hnsw.iterative_scan = relaxed_order").execute(&mut *tx))
					.await
					.map_err(|error| if error == StoreError::CollectionNotFound { StoreError::ExtensionTooOld } else { error })?;
				let rows = timed(query.fetch_all(&mut *tx)).await?;
				Ok(rows.into_iter().map(|(payload, distance)| (payload, view.distance.pg_score(distance))).collect())
			}
			Self::Qdrant(qdrant) => {
				let mut body = serde_json::json!({ "query": vector, "limit": limit, "with_payload": true, "with_vector": false });
				if let Some(name) = &view.vector_name {
					body["using"] = Value::String(name.clone());
				}
				let access = view.access.as_deref().map(|field| qdrant_access(field, labels));
				if let Some(filter) = qdrant_filter(&view.filter, access) {
					body["filter"] = filter;
				}
				let points: QdrantResult<QdrantPoints> = qdrant
					.send(reqwest::Method::POST, &["collections", &view.collection, "points", "query"], Some(&body))
					.await?;
				Ok(points
					.result
					.points
					.into_iter()
					.map(|point| (point.payload, view.distance.qdrant_score(point.score)))
					.collect())
			}
		}
	}

	/// Up to [`MAX_ACCESS_LABELS`] distinct labels in the view's access field, sorted; empty without per-record access.
	///
	/// Qdrant needs a keyword payload index on the field to count them.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be read or the field is unknown.
	pub async fn access_labels(&self, view: &ExternalView) -> Result<Vec<String>, StoreError> {
		let Some(field) = &view.access else {
			return Ok(Vec::new());
		};
		let mut labels: Vec<String> = match self {
			Self::Pgvector(pool) => {
				let table = pg_table(pool, &view.collection).await?;
				let columns = table.columns();
				let (value, path) = pg_access_value(field, &columns, 1).ok_or(StoreError::UnknownField)?;
				let (clause, params) = pg_filter(&view.filter, &columns, 3).ok_or(StoreError::UnknownField)?;
				let sql = format!(
					"SELECT DISTINCT e #>> '{{}}' FROM (SELECT {value} AS v FROM {} t{clause}) r,
					 jsonb_array_elements(CASE jsonb_typeof(r.v) WHEN 'array' THEN r.v WHEN 'string' THEN jsonb_build_array(r.v) ELSE '[]'::jsonb END) AS e
					 WHERE jsonb_typeof(e) = 'string' ORDER BY 1 LIMIT $2",
					table.from
				);
				let query = sqlx::query_scalar::<_, String>(AssertSqlSafe(sql)).bind(path).bind(i64::from(MAX_ACCESS_LABELS));
				timed(params.iter().fold(query, sqlx::query::QueryScalar::bind).fetch_all(pool.as_ref())).await?
			}
			Self::Qdrant(qdrant) => {
				let mut body = serde_json::json!({ "key": field, "limit": MAX_ACCESS_LABELS, "exact": false });
				if let Some(filter) = qdrant_filter(&view.filter, None) {
					body["filter"] = filter;
				}
				let facet: QdrantResult<QdrantFacet> = qdrant
					.send(reqwest::Method::POST, &["collections", &view.collection, "facet"], Some(&body))
					.await?;
				facet.result.hits.into_iter().filter_map(|hit| hit.value.as_str().map(str::to_string)).collect()
			}
		};
		labels.sort();
		labels.dedup();
		Ok(labels)
	}

	/// Creates the `vector` extension and the managed vector table + indexes for `(connection_id, generation)` if missing.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store is not pgvector, lacks pgvector 0.8+ or DDL fails.
	pub async fn ensure_managed(&self, connection_id: &Uuid, generation: i32, dimensions: u32) -> Result<(), StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		if !(1..=super::MAX_DIMENSIONS).contains(&dimensions) {
			return Err(StoreError::BadResponse);
		}
		timed(sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(pool.as_ref())).await?;
		self.health().await?;
		let table = managed_table_name(connection_id, generation);
		let statements = [
			format!(
				"CREATE TABLE IF NOT EXISTS {table} (
					chunk_id UUID PRIMARY KEY,
					knowledge_base_id UUID NOT NULL,
					document_id UUID NOT NULL,
					access TEXT[],
					embedding halfvec({dimensions}) NOT NULL
				)"
			),
			format!("CREATE INDEX IF NOT EXISTS idx_{table}_kb ON {table} (knowledge_base_id)"),
			format!("CREATE INDEX IF NOT EXISTS idx_{table}_doc ON {table} (document_id)"),
			format!("CREATE INDEX IF NOT EXISTS idx_{table}_access ON {table} USING GIN (access)"),
			format!("CREATE INDEX IF NOT EXISTS idx_{table}_hnsw ON {table} USING hnsw (embedding halfvec_cosine_ops)"),
		];
		for sql in statements {
			timed(sqlx::query(AssertSqlSafe(sql)).execute(pool.as_ref())).await?;
		}
		Ok(())
	}

	/// Normalizes chunk vectors and writes them into the managed pgvector table in one transaction, replacing the vectors their documents had there.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when any vector is invalid or the database write fails.
	pub async fn upsert_managed(&self, connection_id: &Uuid, generation: i32, rows: &[ManagedVector]) -> Result<(), StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		if rows.is_empty() {
			return Ok(());
		}
		let table = managed_table_name(connection_id, generation);
		let sql = format!(
			"INSERT INTO {table} (chunk_id, knowledge_base_id, document_id, access, embedding)
			 VALUES ($1, $2, $3, $4, $5::real[]::vector::halfvec)
			 ON CONFLICT (chunk_id) DO UPDATE SET access = EXCLUDED.access, embedding = EXCLUDED.embedding"
		);
		let mut documents: Vec<Uuid> = rows.iter().map(|row| row.document_id).collect();
		documents.sort_unstable();
		documents.dedup();
		let mut tx = timed(pool.begin()).await?;
		let clear = format!("DELETE FROM {table} WHERE document_id = ANY($1)");
		timed(sqlx::query(AssertSqlSafe(clear)).bind(&documents).execute(&mut *tx)).await?;
		for row in rows {
			let mut vector = row.embedding.clone();
			if !super::normalize_vector(&mut vector) {
				return Err(StoreError::BadResponse);
			}
			timed(
				sqlx::query(AssertSqlSafe(sql.clone()))
					.bind(row.chunk_id)
					.bind(row.knowledge_base_id)
					.bind(row.document_id)
					.bind(row.access.as_deref())
					.bind(&vector)
					.execute(&mut *tx),
			)
			.await?;
		}
		timed(tx.commit()).await?;
		Ok(())
	}

	/// Updates the access label array on every stored vector of `document_id` across all generations of `connection_id`.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the database update fails.
	pub async fn update_managed_access(&self, connection_id: &Uuid, document_id: &Uuid, access: Option<&[String]>) -> Result<(), StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		for table in list_managed_tables(pool.as_ref(), connection_id).await? {
			let sql = format!("UPDATE {} SET access = $2 WHERE document_id = $1", quote_ident(&table));
			timed(sqlx::query(AssertSqlSafe(sql)).bind(document_id).bind(access).execute(pool.as_ref())).await?;
		}
		Ok(())
	}

	/// Deletes all stored vectors for `document_id` across every generation table of `connection_id`.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the database delete fails.
	pub async fn delete_managed_document(&self, connection_id: &Uuid, document_id: &Uuid) -> Result<(), StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		for table in list_managed_tables(pool.as_ref(), connection_id).await? {
			let sql = format!("DELETE FROM {} WHERE document_id = $1", quote_ident(&table));
			timed(sqlx::query(AssertSqlSafe(sql)).bind(document_id).execute(pool.as_ref())).await?;
		}
		Ok(())
	}

	/// Drops managed vector tables of `connection_id` from generations before `keep_generation` once re-indexing is complete.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when a drop fails.
	pub async fn drop_old_managed_tables(&self, connection_id: &Uuid, keep_generation: i32) -> Result<(), StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		let prefix = managed_table_prefix(connection_id);
		for table in list_managed_tables(pool.as_ref(), connection_id).await? {
			let generation = table.strip_prefix(&prefix).and_then(|suffix| suffix.parse::<i32>().ok());
			if generation.is_some_and(|generation| generation < keep_generation) {
				let sql = format!("DROP TABLE IF EXISTS {}", quote_ident(&table));
				timed(sqlx::query(AssertSqlSafe(sql)).execute(pool.as_ref())).await?;
			}
		}
		Ok(())
	}

	/// Searches a managed knowledge base's vectors, filtering by `knowledge_base_id` and per-document `access` inside the query.
	///
	/// # Errors
	///
	/// Returns a `StoreError` when the store cannot be read or pgvector is older than 0.8.
	pub async fn search_managed(&self, query: &ManagedSearch<'_>) -> Result<Vec<(Uuid, f64)>, StoreError> {
		let Self::Pgvector(pool) = self else {
			return Err(StoreError::BadResponse);
		};
		let limit = query.limit.clamp(1, MAX_SEARCH_HITS);
		let mut query_vector = query.vector.to_vec();
		if !super::normalize_vector(&mut query_vector) {
			return Err(StoreError::BadResponse);
		}
		let table = managed_table_name(query.connection_id, query.generation);
		let exists: bool = timed(sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL").bind(&table).fetch_one(pool.as_ref())).await?;
		if !exists {
			return Ok(Vec::new());
		}
		let sql = format!(
			"SELECT chunk_id, (1.0 - (embedding <=> $2::real[]::vector::halfvec))::float8
			 FROM {table}
			 WHERE knowledge_base_id = $1 AND (access IS NULL OR access && $3::text[])
			 ORDER BY embedding <=> $2::real[]::vector::halfvec
			 LIMIT $4"
		);
		let mut tx = timed(pool.begin()).await?;
		timed(sqlx::query("SET LOCAL hnsw.iterative_scan = relaxed_order").execute(&mut *tx))
			.await
			.map_err(|error| if error == StoreError::CollectionNotFound { StoreError::ExtensionTooOld } else { error })?;
		let rows = timed(
			sqlx::query_as::<_, (Uuid, f64)>(AssertSqlSafe(sql))
				.bind(query.knowledge_base_id)
				.bind(&query_vector)
				.bind(query.labels)
				.bind(i64::from(limit))
				.fetch_all(&mut *tx),
		)
		.await?;
		Ok(rows)
	}
}

async fn list_managed_tables(pool: &PgPool, connection_id: &Uuid) -> Result<Vec<String>, StoreError> {
	let prefix = format!("{}%", managed_table_prefix(connection_id));
	timed(
		sqlx::query_scalar::<_, String>(
			"SELECT c.relname::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
			 WHERE n.nspname = current_schema() AND c.relkind = 'r' AND c.relname LIKE $1
			 ORDER BY c.relname",
		)
		.bind(prefix)
		.fetch_all(pool),
	)
	.await
}

async fn pg_table(pool: &PgPool, collection: &str) -> Result<PgTable, StoreError> {
	let (oid, schema, name) = timed(
		sqlx::query_as::<_, (Oid, String, String)>(
			"SELECT c.oid, n.nspname::text, c.relname::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
			 WHERE c.oid = to_regclass($1) AND c.relkind IN ('r', 'v', 'm', 'p', 'f')
			 AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_toast%'
			 AND EXISTS (SELECT 1 FROM pg_attribute a JOIN pg_type ty ON ty.oid = a.atttypid
			             WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped AND ty.typname IN ('vector', 'halfvec'))",
		)
		.bind(collection)
		.fetch_optional(pool),
	)
	.await?
	.ok_or(StoreError::CollectionNotFound)?;
	let columns = timed(
		sqlx::query_as::<_, (String, String, i32)>(
			"SELECT a.attname::text, t.typname::text, a.atttypmod FROM pg_attribute a JOIN pg_type t ON t.oid = a.atttypid
			 WHERE a.attrelid = $1 AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum",
		)
		.bind(oid)
		.fetch_all(pool),
	)
	.await?;
	Ok(PgTable {
		from: format!("{}.{}", quote_ident(&schema), quote_ident(&name)),
		columns,
	})
}

async fn pg_vector_size(pool: &PgPool, table: &PgTable, column: &str, kind: &str) -> Result<Option<u32>, StoreError> {
	let column = quote_ident(column);
	let cast = if kind == "halfvec" { "::vector" } else { "" };
	let sql = format!("SELECT vector_dims(t.{column}{cast}) FROM {} t WHERE t.{column} IS NOT NULL LIMIT 1", table.from);
	let size: Option<i32> = timed(sqlx::query_scalar(AssertSqlSafe(sql)).fetch_optional(pool)).await?;
	Ok(size.and_then(|size| u32::try_from(size).ok()))
}

fn qdrant_vectors(config: &Value) -> Vec<VectorInfo> {
	let info = |name: Option<String>, params: &Value| VectorInfo {
		name,
		size: params.get("size").and_then(Value::as_u64).and_then(|size| u32::try_from(size).ok()),
		distance: params.get("distance").and_then(Value::as_str).and_then(Distance::from_qdrant),
	};
	match config.as_object() {
		Some(map) if map.contains_key("size") => vec![info(None, config)],
		Some(map) => map.iter().map(|(name, params)| info(Some(name.clone()), params)).collect(),
		None => Vec::new(),
	}
}

/// Pools of external pgvector stores by connection, with a hash of everything they connect with, so searches reuse
/// connections and a changed address, secret or mode replaces the pool. Idle connections close after [`POOL_IDLE`].
static PG_POOLS: LazyLock<ArcSwap<HashMap<Uuid, CachedPool>>> = LazyLock::new(ArcSwap::default);

/// Counts [`forget_pool`] calls, so a pool that finished connecting after its connection was changed or deleted is not cached.
static POOLS_FORGOTTEN: AtomicU64 = AtomicU64::new(0);

/// A pool and the hash of the settings it connects with.
type CachedPool = ([u8; 32], PgPool);

/// Drops the cached pool of a connection that was changed or deleted.
pub fn forget_pool(connection_id: &Uuid) {
	POOLS_FORGOTTEN.fetch_add(1, Ordering::SeqCst);
	PG_POOLS.rcu(|pools| {
		let mut pools = HashMap::clone(pools);
		pools.remove(connection_id);
		pools
	});
}

/// Caches `pool` unless a pool was forgotten since `epoch`; returns the cached pool when another search cached one first.
fn cache_pool(connection_id: Uuid, key: [u8; 32], pool: PgPool, epoch: u64) -> PgPool {
	let mut cached = pool.clone();
	PG_POOLS.rcu(|pools| {
		if POOLS_FORGOTTEN.load(Ordering::SeqCst) != epoch {
			return HashMap::clone(pools);
		}
		if let Some((existing, existing_pool)) = pools.get(&connection_id)
			&& *existing == key
		{
			cached = existing_pool.clone();
			return HashMap::clone(pools);
		}
		cached = pool.clone();
		let mut pools = HashMap::clone(pools);
		pools.insert(connection_id, (key, pool.clone()));
		pools
	});
	cached
}

fn pool_key(url: &str, password: Option<&str>, mode: ConnectionMode) -> [u8; 32] {
	let mut hasher = Sha256::new();
	for part in [url, password.unwrap_or_default(), if mode == ConnectionMode::ReadOnly { "read_only" } else { "managed" }] {
		hasher.update(part.as_bytes());
		hasher.update([0]);
	}
	hasher.finalize().into()
}

async fn connect_postgres(connection: &RagConnection, url: &str, password: Option<&str>, trusted: bool) -> Result<VectorStore<'static>, StoreError> {
	let mode = connection.mode;
	let epoch = POOLS_FORGOTTEN.load(Ordering::SeqCst);
	let mut options = PgConnectOptions::from_str(url).map_err(|_| StoreError::InvalidUrl)?;
	if !trusted {
		if options.get_socket().is_some() {
			return Err(StoreError::InvalidUrl);
		}
		check_host(options.get_host(), options.get_port()).await?;
	}
	let key = pool_key(url, password, mode);
	if let Some((_, pool)) = PG_POOLS.load().get(&connection.id).filter(|(cached, _)| *cached == key) {
		return Ok(VectorStore::Pgvector(Cow::Owned(pool.clone())));
	}
	if !trusted || password.is_some() {
		options = options.password(password.unwrap_or_default());
	}
	if mode == ConnectionMode::ReadOnly {
		options = options.options([("default_transaction_read_only", "on")]);
	}
	let pool = PgPoolOptions::new()
		.max_connections(POOL_SIZE)
		.min_connections(0)
		.idle_timeout(POOL_IDLE)
		.acquire_timeout(TIMEOUT)
		.connect_with(options)
		.await
		.map_err(|error| store_error(&error))?;
	Ok(VectorStore::Pgvector(Cow::Owned(cache_pool(connection.id, key, pool, epoch))))
}

/// Refuses hosts that resolve to blocked addresses. sqlx resolves the host again when it connects, so a DNS answer that
/// changes in between is not caught; pin the address once sqlx can keep TLS host verification with it.
async fn check_host(host: &str, port: u16) -> Result<(), StoreError> {
	if host.is_empty() || host.starts_with('/') {
		return Err(StoreError::InvalidUrl);
	}
	let addresses: Vec<_> = tokio::time::timeout(TIMEOUT, tokio::net::lookup_host((host.trim_matches(['[', ']']), port)))
		.await
		.map_err(|_| StoreError::Unreachable)?
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
		let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
		self.send(reqwest::Method::GET, &segments, None).await
	}

	async fn send<T: serde::de::DeserializeOwned>(&self, method: reqwest::Method, segments: &[&str], body: Option<&Value>) -> Result<T, StoreError> {
		if segments.iter().any(|segment| matches!(*segment, "." | "..")) {
			return Err(StoreError::InvalidUrl);
		}
		let mut url = self.base.clone();
		if !segments.is_empty() {
			url.path_segments_mut().map_err(|()| StoreError::InvalidUrl)?.pop_if_empty().extend(segments);
		}
		let mut request = self.client.request(method, url);
		if let Some(key) = &self.api_key {
			request = request.header("api-key", key);
		}
		if let Some(body) = body {
			request = request.json(body);
		}
		let response = request.send().await.map_err(|_| StoreError::Unreachable)?;
		match response.status() {
			reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => return Err(StoreError::Unauthorized),
			reqwest::StatusCode::NOT_FOUND if !segments.is_empty() => return Err(StoreError::CollectionNotFound),
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
	let code = error.as_database_error().and_then(sqlx::error::DatabaseError::code);
	match code.as_deref() {
		Some("28P01" | "28000" | "42501") => StoreError::Unauthorized,
		Some("42601" | "42602" | "42704") => StoreError::CollectionNotFound,
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
