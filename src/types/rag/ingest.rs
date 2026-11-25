//! Background ingest and deletion worker for managed knowledge bases.

use super::{
	DocumentStatus, ManagedVector, ModelTargets, RagConfig, RagConnection, RagError, TextChunk, VectorStore, check_embedding_allowed, check_search_language, chunk_document,
	embed_batch,
};
use crate::types::JobState;
use crate::utils::extract::{self, DocumentKind};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, sqlx::FromRow)]
struct ClaimedDocument {
	id: Uuid,
	knowledge_base_id: Uuid,
	filename: String,
	mime_type: String,
	text: Option<String>,
	source_bytes: Option<Vec<u8>>,
	index_generation: i32,
	uploaded_by: Option<Uuid>,
	reindex_by: Option<Uuid>,
	connection_id: Uuid,
	search_language: String,
}

#[derive(Debug, sqlx::FromRow)]
struct DeletingDocument {
	id: Uuid,
	knowledge_base_id: Uuid,
	connection_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ExistingChunk {
	id: Uuid,
	heading_path: Option<String>,
	content: String,
	token_count: i32,
}

/// Runs one worker loop: drains pending/deleting work and sleeps until notified or [`POLL_INTERVAL`] elapses.
pub async fn rag_ingest_worker(state: Arc<JobState>) {
	loop {
		match process_one(&state).await {
			Ok(true) => continue,
			Ok(false) => {}
			Err(error) => {
				tracing::error!(%error, "[JOBS] RAG ingest worker error");
				tokio::time::sleep(Duration::from_secs(5)).await;
				continue;
			}
		}
		tokio::select! {
			() = state.rag_ingest_notify.notified() => {}
			() = tokio::time::sleep(POLL_INTERVAL) => {}
		}
	}
}

/// Performs stuck-job recovery, failed-upload byte cleanup, and processes at most one `deleting` or `pending` document.
///
/// Returns `true` when a document was processed so callers can loop until the queue is drained.
///
/// # Errors
///
/// Returns a database error when claiming or maintenance queries fail.
pub async fn process_one(state: &JobState) -> Result<bool, sqlx::Error> {
	reap_stuck_and_cleanup(&state.db).await?;
	if let Some(doc) = claim_deleting(&state.db).await? {
		process_deleting(state, &doc).await;
		return Ok(true);
	}
	if let Some(doc) = claim_pending(&state.db).await? {
		if let Err(reason) = ingest_document(state, &doc).await {
			let _ = mark_failed(&state.db, &doc.id, reason).await;
		}
		return Ok(true);
	}
	Ok(false)
}

async fn reap_stuck_and_cleanup(pool: &PgPool) -> Result<(), sqlx::Error> {
	sqlx::query(
		"UPDATE rag_documents SET status = 'pending', locked_at = NULL, updated_at = NOW()
		 WHERE status = 'processing' AND locked_at < NOW() - INTERVAL '10 minutes'",
	)
	.execute(pool)
	.await?;
	sqlx::query(
		"UPDATE rag_documents SET locked_at = NULL, updated_at = NOW()
		 WHERE status = 'deleting' AND locked_at < NOW() - INTERVAL '10 minutes'",
	)
	.execute(pool)
	.await?;
	sqlx::query(
		"UPDATE rag_documents SET source_bytes = NULL, updated_at = NOW()
		 WHERE status = 'failed' AND source_bytes IS NOT NULL AND updated_at < NOW() - INTERVAL '7 days'",
	)
	.execute(pool)
	.await?;
	Ok(())
}

async fn claim_deleting(pool: &PgPool) -> Result<Option<DeletingDocument>, sqlx::Error> {
	let mut tx = pool.begin().await?;
	let row: Option<DeletingDocument> = sqlx::query_as(
		"SELECT d.id, d.knowledge_base_id, kb.connection_id
		 FROM rag_documents d
		 JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id
		 WHERE d.status = 'deleting' AND d.locked_at IS NULL
		 ORDER BY d.updated_at ASC, d.id ASC
		 LIMIT 1
		 FOR UPDATE OF d SKIP LOCKED",
	)
	.fetch_optional(&mut *tx)
	.await?;
	if let Some(doc) = &row {
		sqlx::query("UPDATE rag_documents SET locked_at = NOW() WHERE id = $1").bind(doc.id).execute(&mut *tx).await?;
	}
	tx.commit().await?;
	Ok(row)
}

async fn process_deleting(state: &JobState, doc: &DeletingDocument) {
	if let Err(error) = finish_deleting(state, doc).await {
		tracing::warn!(document_id = %doc.id, %error, "failed to finish deleting document; will retry");
	}
}

async fn finish_deleting(state: &JobState, doc: &DeletingDocument) -> Result<(), String> {
	delete_document_vectors(state, doc).await?;
	remove_deleted_document(&state.db, doc).await.map_err(|error| error.to_string())
}

async fn delete_document_vectors(state: &JobState, doc: &DeletingDocument) -> Result<(), String> {
	let config = RagConfig::get(&state.db).await.map_err(|error| error.to_string())?;
	let Some(connection) = RagConnection::find(&state.db, &doc.connection_id).await.map_err(|error| error.to_string())? else {
		return Ok(());
	};
	let env_url = config.env_url_for(&connection);
	let store = VectorStore::connect(&connection, &state.db, env_url.as_deref())
		.await
		.map_err(|error| error.to_string())?;
	store
		.delete_managed_document(&connection.id, &doc.id)
		.await
		.map_err(|error| error.to_string())
}

async fn remove_deleted_document(pool: &PgPool, doc: &DeletingDocument) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;
	sqlx::query("DELETE FROM rag_documents WHERE id = $1").bind(doc.id).execute(&mut *tx).await?;
	sqlx::query(
		"DELETE FROM knowledge_bases kb
		 WHERE kb.id = $1 AND kb.deleted_at IS NOT NULL
		   AND NOT EXISTS (SELECT 1 FROM rag_documents d WHERE d.knowledge_base_id = kb.id)",
	)
	.bind(doc.knowledge_base_id)
	.execute(&mut *tx)
	.await?;
	tx.commit().await
}

async fn claim_pending(pool: &PgPool) -> Result<Option<ClaimedDocument>, sqlx::Error> {
	let mut tx = pool.begin().await?;
	let row: Option<ClaimedDocument> = sqlx::query_as(
		"SELECT d.id, d.knowledge_base_id, d.filename, d.mime_type, d.text, d.source_bytes, d.index_generation,
		        d.uploaded_by, d.reindex_by, kb.connection_id, kb.search_language
		 FROM rag_documents d
		 JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id
		 WHERE d.status = 'pending' AND kb.deleted_at IS NULL
		 ORDER BY d.created_at ASC, d.id ASC
		 LIMIT 1
		 FOR UPDATE OF d SKIP LOCKED",
	)
	.fetch_optional(&mut *tx)
	.await?;
	if let Some(doc) = &row {
		sqlx::query("UPDATE rag_documents SET status = 'processing', locked_at = NOW(), error = NULL, updated_at = NOW() WHERE id = $1")
			.bind(doc.id)
			.execute(&mut *tx)
			.await?;
	}
	tx.commit().await?;
	Ok(row)
}

struct IngestTarget<'a> {
	config: RagConfig,
	connection: RagConnection,
	embedding: super::EmbeddingSettings,
	billing_user: Uuid,
	store: VectorStore<'a>,
}

async fn prepare_ingest_target<'a>(state: &'a JobState, doc: &ClaimedDocument) -> Result<IngestTarget<'a>, &'static str> {
	let config = RagConfig::get(&state.db).await.map_err(|_| "store_failed")?;
	let connection = RagConnection::find(&state.db, &doc.connection_id)
		.await
		.map_err(|_| "store_failed")?
		.ok_or("connection_not_ready")?;
	let embedding = connection.embedding_settings().ok_or("connection_not_ready")?;
	let dimensions = connection.vector_size.and_then(|size| u32::try_from(size).ok()).ok_or("connection_not_ready")?;
	let targets = ModelTargets::load(&state.db, &[doc.knowledge_base_id])
		.await
		.map_err(|_| "store_failed")?
		.remove(&doc.knowledge_base_id)
		.unwrap_or_default();
	check_embedding_allowed(&state.db, &targets, &embedding.model_id)
		.await
		.map_err(|_| "embedding_model_not_allowed")?;
	let billing_user = doc.reindex_by.or(doc.uploaded_by).ok_or("embedding_failed")?;
	let env_url = config.env_url_for(&connection);
	let store = VectorStore::connect(&connection, &state.db, env_url.as_deref()).await.map_err(|_| "store_failed")?;
	store
		.ensure_managed(&connection.id, connection.index_generation, dimensions)
		.await
		.map_err(|_| "store_failed")?;
	Ok(IngestTarget {
		config,
		connection,
		embedding,
		billing_user,
		store,
	})
}

async fn ingest_document(state: &JobState, doc: &ClaimedDocument) -> Result<(), &'static str> {
	let target = prepare_ingest_target(state, doc).await?;
	let existing_chunks = load_existing_chunks(&state.db, &doc.id).await.map_err(|_| "store_failed")?;
	if doc.source_bytes.is_none() && doc.index_generation < target.connection.index_generation && !existing_chunks.is_empty() {
		let ctx = ReindexContext {
			doc,
			connection: &target.connection,
			store: &target.store,
			billing_user: &target.billing_user,
		};
		reindex_existing_chunks(state, &ctx, &existing_chunks).await?;
		maybe_cleanup_old_tables(state, &target.connection, &target.store).await;
		return Ok(());
	}
	index_fresh_document(state, doc, &target).await?;
	maybe_cleanup_old_tables(state, &target.connection, &target.store).await;
	Ok(())
}

async fn index_fresh_document(state: &JobState, doc: &ClaimedDocument, target: &IngestTarget<'_>) -> Result<(), &'static str> {
	let text = extract_or_reuse_text(&state.db, doc).await?;
	let chunks = chunk_document(&text, target.config.chunk_config());
	if chunks.is_empty() {
		return Err("empty");
	}
	let inputs: Vec<String> = chunks.iter().map(|chunk| chunk.embed_text(&doc.filename)).collect();
	let vectors = embed_batch(state, &target.billing_user, &target.embedding, &inputs, "rag_ingest")
		.await
		.map_err(rag_doc_error)?;
	let language = check_search_language(&doc.search_language).unwrap_or("simple");

	let Some((chunk_ids, latest_access)) = store_chunks(&state.db, doc, &chunks, language).await.map_err(|_| "store_failed")? else {
		return Ok(());
	};
	let total_tokens: i32 = chunks.iter().map(|chunk| chunk.token_count).fold(0i32, i32::saturating_add);
	let chunk_count = i32::try_from(chunks.len()).unwrap_or(i32::MAX);
	let rows: Vec<ManagedVector> = chunk_ids
		.into_iter()
		.zip(vectors)
		.map(|(chunk_id, embedding)| ManagedVector {
			chunk_id,
			knowledge_base_id: doc.knowledge_base_id,
			document_id: doc.id,
			access: latest_access.clone(),
			embedding,
		})
		.collect();
	target
		.store
		.upsert_managed(&target.connection.id, target.connection.index_generation, &rows)
		.await
		.map_err(|_| "store_failed")?;

	if !finalize_ready(&state.db, &doc.id, chunk_count, total_tokens, target.connection.index_generation)
		.await
		.map_err(|_| "store_failed")?
	{
		let _ = target.store.delete_managed_document(&target.connection.id, &doc.id).await;
	}
	Ok(())
}

async fn extract_or_reuse_text(pool: &PgPool, doc: &ClaimedDocument) -> Result<String, &'static str> {
	if let Some(bytes) = &doc.source_bytes {
		let kind = DocumentKind::from_mime(&doc.mime_type).ok_or("extract_failed")?;
		let text = extract::extract(kind, bytes).await.map_err(extract::ExtractError::reason)?;
		let pages = if kind == DocumentKind::Pdf { extract::page_count(&text) } else { None };
		sqlx::query("UPDATE rag_documents SET text = $2, page_count = $3, source_bytes = NULL, updated_at = NOW() WHERE id = $1")
			.bind(doc.id)
			.bind(&text)
			.bind(pages)
			.execute(pool)
			.await
			.map_err(|_| "store_failed")?;
		Ok(text)
	} else {
		doc.text.clone().filter(|text| !text.trim().is_empty()).ok_or("empty")
	}
}

async fn load_existing_chunks(pool: &PgPool, document_id: &Uuid) -> Result<Vec<ExistingChunk>, sqlx::Error> {
	sqlx::query_as("SELECT id, heading_path, content, token_count FROM rag_chunks WHERE document_id = $1 ORDER BY ordinal ASC")
		.bind(document_id)
		.fetch_all(pool)
		.await
}

struct ReindexContext<'a> {
	doc: &'a ClaimedDocument,
	connection: &'a RagConnection,
	store: &'a VectorStore<'a>,
	billing_user: &'a Uuid,
}

async fn reindex_existing_chunks(state: &JobState, ctx: &ReindexContext<'_>, chunks: &[ExistingChunk]) -> Result<(), &'static str> {
	let embedding = ctx.connection.embedding_settings().ok_or("connection_not_ready")?;
	let inputs: Vec<String> = chunks
		.iter()
		.map(|chunk| match &chunk.heading_path {
			Some(path) if !path.is_empty() => format!("{} › {path}\n\n{}", ctx.doc.filename, chunk.content),
			_ => format!("{}\n\n{}", ctx.doc.filename, chunk.content),
		})
		.collect();
	let vectors = embed_batch(state, ctx.billing_user, &embedding, &inputs, "rag_ingest").await.map_err(rag_doc_error)?;
	let current_access: Option<Option<Vec<String>>> = sqlx::query_scalar("SELECT access FROM rag_documents WHERE id = $1 AND status = 'processing'")
		.bind(ctx.doc.id)
		.fetch_optional(&state.db)
		.await
		.map_err(|_| "store_failed")?;
	let Some(access) = current_access else {
		return Ok(());
	};
	let rows: Vec<ManagedVector> = chunks
		.iter()
		.zip(vectors)
		.map(|(chunk, embedding)| ManagedVector {
			chunk_id: chunk.id,
			knowledge_base_id: ctx.doc.knowledge_base_id,
			document_id: ctx.doc.id,
			access: access.clone(),
			embedding,
		})
		.collect();
	ctx.store
		.upsert_managed(&ctx.connection.id, ctx.connection.index_generation, &rows)
		.await
		.map_err(|_| "store_failed")?;
	let total_tokens: i32 = chunks.iter().map(|chunk| chunk.token_count).fold(0i32, i32::saturating_add);
	let chunk_count = i32::try_from(chunks.len()).unwrap_or(i32::MAX);
	if !finalize_ready(&state.db, &ctx.doc.id, chunk_count, total_tokens, ctx.connection.index_generation)
		.await
		.map_err(|_| "store_failed")?
	{
		let _ = ctx.store.delete_managed_document(&ctx.connection.id, &ctx.doc.id).await;
	}
	Ok(())
}

async fn store_chunks(
	pool: &PgPool,
	doc: &ClaimedDocument,
	chunks: &[TextChunk],
	language: &'static str,
) -> Result<Option<(Vec<Uuid>, Option<Vec<String>>)>, sqlx::Error> {
	let mut tx = pool.begin().await?;
	let current: Option<(DocumentStatus, Option<Vec<String>>)> = sqlx::query_as("SELECT status, access FROM rag_documents WHERE id = $1 FOR UPDATE")
		.bind(doc.id)
		.fetch_optional(&mut *tx)
		.await?;
	let Some((DocumentStatus::Processing, access)) = current else {
		return Ok(None);
	};
	sqlx::query("DELETE FROM rag_chunks WHERE document_id = $1").bind(doc.id).execute(&mut *tx).await?;
	let mut ids = Vec::with_capacity(chunks.len());
	for chunk in chunks {
		let id: Uuid = sqlx::query_scalar(
			"INSERT INTO rag_chunks (document_id, knowledge_base_id, ordinal, heading_path, page, content, content_hash, token_count, search, access)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, to_tsvector($9::regconfig, $6), $10)
			 RETURNING id",
		)
		.bind(doc.id)
		.bind(doc.knowledge_base_id)
		.bind(chunk.ordinal)
		.bind(&chunk.heading_path)
		.bind(chunk.page)
		.bind(&chunk.content)
		.bind(&chunk.content_hash)
		.bind(chunk.token_count)
		.bind(language)
		.bind(access.as_deref())
		.fetch_one(&mut *tx)
		.await?;
		ids.push(id);
	}
	tx.commit().await?;
	Ok(Some((ids, access)))
}

async fn finalize_ready(pool: &PgPool, document_id: &Uuid, chunk_count: i32, token_count: i32, generation: i32) -> Result<bool, sqlx::Error> {
	let updated = sqlx::query(
		"UPDATE rag_documents
		 SET status = 'ready', chunk_count = $2, token_count = $3, index_generation = $4, reindex_by = NULL, locked_at = NULL, error = NULL, updated_at = NOW()
		 WHERE id = $1 AND status = 'processing'",
	)
	.bind(document_id)
	.bind(chunk_count)
	.bind(token_count)
	.bind(generation)
	.execute(pool)
	.await?;
	Ok(updated.rows_affected() > 0)
}

async fn mark_failed(pool: &PgPool, document_id: &Uuid, reason: &'static str) -> Result<(), sqlx::Error> {
	sqlx::query(
		"UPDATE rag_documents SET status = 'failed', error = $2, locked_at = NULL, updated_at = NOW()
		 WHERE id = $1 AND status = 'processing'",
	)
	.bind(document_id)
	.bind(reason)
	.execute(pool)
	.await?;
	Ok(())
}

async fn maybe_cleanup_old_tables(state: &JobState, connection: &RagConnection, store: &VectorStore<'_>) {
	if matches!(connection.is_reindexing(&state.db).await, Ok(false)) {
		let _ = store.drop_old_managed_tables(&connection.id, connection.index_generation).await;
	}
}

fn rag_doc_error(error: RagError) -> &'static str {
	match error {
		RagError::BudgetExceeded => "budget_exceeded",
		RagError::EmbeddingModelNotAllowed => "embedding_model_not_allowed",
		RagError::ConnectionNotReady => "connection_not_ready",
		RagError::Store(_) => "store_failed",
		_ => "embedding_failed",
	}
}
