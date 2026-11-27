//! Uploaded documents of managed knowledge bases, per-user upload quotas and per-document "Limit access" labels.

use super::{
	DocumentStatus, KnowledgeActor, KnowledgeBase, ModelTargets, RagConfig, RagConnection, RagDocumentResponse, RagError, VectorStore, check_embedding_allowed, sha256_hex,
};
use crate::logging::AuditLog;
use crate::types::{AssignmentKind, AssignmentTarget, AssignmentsResponse, AuditEntry, JobState, ReplaceAssignmentsRequest};
use crate::utils::extract::{DocumentKind, MAX_UPLOAD_BYTES};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

const AUDIT_TARGET: &str = "rag_document";

/// A stored row of `rag_documents` without its `source_bytes` or `text` payloads.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RagDocument {
	pub id: Uuid,
	pub knowledge_base_id: Uuid,
	pub filename: String,
	pub mime_type: String,
	pub size_bytes: i64,
	pub status: DocumentStatus,
	pub error: Option<String>,
	pub page_count: Option<i32>,
	pub chunk_count: i32,
	pub token_count: i32,
	pub access: Option<Vec<String>>,
	pub can_retry: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Internal labels representing `user_id` (`user:<id>`, `team:<id>`, `role:<id>`), loaded live from the database.
///
/// # Errors
///
/// Returns a database error when the query fails.
pub async fn user_identity_labels(pool: &PgPool, user_id: &Uuid) -> Result<Vec<String>, sqlx::Error> {
	let teams: Vec<Uuid> = sqlx::query_scalar("SELECT team_id FROM team_members WHERE user_id = $1")
		.bind(user_id)
		.fetch_all(pool)
		.await?;
	let roles: Vec<Uuid> = sqlx::query_scalar("SELECT role_id FROM user_roles WHERE user_id = $1")
		.bind(user_id)
		.fetch_all(pool)
		.await?;
	let mut labels = Vec::with_capacity(1 + teams.len() + roles.len());
	labels.push(format!("user:{user_id}"));
	for team in teams {
		labels.push(format!("team:{team}"));
	}
	for role in roles {
		labels.push(format!("role:{role}"));
	}
	labels.sort();
	labels.dedup();
	Ok(labels)
}

impl RagDocument {
	/// Target kinds accepted by per-document "Limit access".
	pub const ACCESS_KINDS: [AssignmentKind; 3] = [AssignmentKind::Team, AssignmentKind::Role, AssignmentKind::User];

	/// The documents `actor` may see in a managed knowledge base: editors see all, everyone else those their labels grant.
	///
	/// # Errors
	///
	/// Returns `NotManaged` or a database error.
	pub async fn visible_to(pool: &PgPool, actor: &KnowledgeActor, base: &KnowledgeBase) -> Result<Vec<Self>, RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		let labels = if actor.can_edit(base) { None } else { Some(user_identity_labels(pool, &actor.user_id).await?) };
		Ok(Self::list(pool, &base.id, labels.as_deref()).await?)
	}

	/// Lists documents of a managed knowledge base that are not being deleted, newest first; with `labels`, only those the labels grant.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool, knowledge_base_id: &Uuid, labels: Option<&[String]>) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			"SELECT id, knowledge_base_id, filename, mime_type, size_bytes, status, error, page_count, chunk_count, token_count, access,
			        (status = 'failed' AND (source_bytes IS NOT NULL OR text IS NOT NULL)) AS can_retry,
			        created_at, updated_at
			 FROM rag_documents
			 WHERE knowledge_base_id = $1 AND status <> 'deleting' AND ($2::text[] IS NULL OR access IS NULL OR access && $2)
			 ORDER BY created_at DESC, id DESC",
		)
		.bind(knowledge_base_id)
		.bind(labels)
		.fetch_all(pool)
		.await
	}

	/// Loads one document of a managed knowledge base.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, knowledge_base_id: &Uuid, document_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			"SELECT id, knowledge_base_id, filename, mime_type, size_bytes, status, error, page_count, chunk_count, token_count, access,
			        (status = 'failed' AND (source_bytes IS NOT NULL OR text IS NOT NULL)) AS can_retry,
			        created_at, updated_at
			 FROM rag_documents
			 WHERE knowledge_base_id = $1 AND id = $2",
		)
		.bind(knowledge_base_id)
		.bind(document_id)
		.fetch_optional(pool)
		.await
	}

	/// Stores raw uploaded bytes for background extraction, chunking and embedding after checking type, size, quotas and model access.
	///
	/// # Errors
	///
	/// Returns validation, permission, quota, rate-limit or database errors.
	pub async fn upload(state: &JobState, actor: &KnowledgeActor, base: &KnowledgeBase, filename: &str, bytes: &[u8]) -> Result<Self, RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		if !actor.can_edit(base) {
			return Err(RagError::Forbidden);
		}
		let filename = sanitize_filename(filename)?;
		if bytes.is_empty() {
			return Err(RagError::EmptyFile);
		}
		if bytes.len() > MAX_UPLOAD_BYTES {
			return Err(RagError::FileTooLarge);
		}
		let kind = DocumentKind::detect(filename, bytes).ok_or(RagError::UnsupportedFile)?;
		let connection = RagConnection::find(&state.db, &base.connection_id).await?.ok_or(RagError::NotFound)?;
		let embedding = connection.embedding_settings().ok_or(RagError::ConnectionNotReady)?;
		if connection.vector_size.is_none() {
			return Err(RagError::ConnectionNotReady);
		}
		let targets = ModelTargets::load(&state.db, &[base.id]).await?.remove(&base.id).unwrap_or_default();
		check_embedding_allowed(&state.db, &targets, &embedding.model_id).await?;
		let config = RagConfig::get(&state.db).await?;

		let hash = sha256_hex(bytes);
		let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
		let mut tx = state.db.begin().await?;
		sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
			.bind(format!("rag_documents:{}", actor.user_id))
			.execute(&mut *tx)
			.await?;
		check_quotas(&mut tx, &config, &actor.user_id, bytes.len()).await?;
		let doc = sqlx::query_as::<_, Self>(
			"INSERT INTO rag_documents (knowledge_base_id, filename, mime_type, size_bytes, sha256, source_bytes, index_generation, uploaded_by)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
			 RETURNING id, knowledge_base_id, filename, mime_type, size_bytes, status, error, page_count, chunk_count, token_count, access,
			           FALSE AS can_retry, created_at, updated_at",
		)
		.bind(base.id)
		.bind(filename)
		.bind(kind.mime_type())
		.bind(size)
		.bind(&hash)
		.bind(bytes)
		.bind(connection.index_generation)
		.bind(actor.user_id)
		.fetch_one(&mut *tx)
		.await?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_document_uploaded",
				actor_id: &actor.user_id,
				target_type: AUDIT_TARGET,
				target_id: &doc.id,
				metadata: serde_json::json!({ "knowledge_base_id": base.id, "filename": filename, "size_bytes": size }),
			},
		)
		.await?;
		tx.commit().await?;
		state.rag_ingest_notify.notify_one();
		Ok(doc)
	}

	/// Marks a document `deleting` so the worker removes its vectors from the vector store before deleting the row.
	///
	/// # Errors
	///
	/// Returns `NotManaged`, `Forbidden`, `NotFound` or a database error.
	pub async fn mark_deleting(state: &JobState, actor: &KnowledgeActor, base: &KnowledgeBase, document_id: &Uuid) -> Result<(), RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		if !actor.can_edit(base) {
			return Err(RagError::Forbidden);
		}
		let mut tx = state.db.begin().await?;
		let filename: Option<String> = sqlx::query_scalar(
			"UPDATE rag_documents SET status = 'deleting', source_bytes = NULL, locked_at = NULL, updated_at = NOW()
			 WHERE id = $1 AND knowledge_base_id = $2
			 RETURNING filename",
		)
		.bind(document_id)
		.bind(base.id)
		.fetch_optional(&mut *tx)
		.await?;
		let Some(filename) = filename else {
			return Err(RagError::NotFound);
		};
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_document_deleted",
				actor_id: &actor.user_id,
				target_type: AUDIT_TARGET,
				target_id: document_id,
				metadata: serde_json::json!({ "knowledge_base_id": base.id, "filename": filename }),
			},
		)
		.await?;
		tx.commit().await?;
		state.rag_ingest_notify.notify_one();
		Ok(())
	}

	/// Re-queues a failed document whose source bytes or extracted text are still stored.
	///
	/// # Errors
	///
	/// Returns `NotManaged`, `Forbidden`, `NotFound`, `CannotRetry` or a database error.
	pub async fn retry(state: &JobState, actor: &KnowledgeActor, base: &KnowledgeBase, document_id: &Uuid) -> Result<Self, RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		if !actor.can_edit(base) {
			return Err(RagError::Forbidden);
		}
		let existing = Self::find(&state.db, &base.id, document_id).await?.ok_or(RagError::NotFound)?;
		if !existing.can_retry {
			return Err(RagError::CannotRetry);
		}
		let updated = sqlx::query_as::<_, Self>(
			"UPDATE rag_documents SET status = 'pending', error = NULL, locked_at = NULL, updated_at = NOW()
			 WHERE id = $1 AND knowledge_base_id = $2 AND status = 'failed' AND (source_bytes IS NOT NULL OR text IS NOT NULL)
			 RETURNING id, knowledge_base_id, filename, mime_type, size_bytes, status, error, page_count, chunk_count, token_count, access,
			           FALSE AS can_retry, created_at, updated_at",
		)
		.bind(document_id)
		.bind(base.id)
		.fetch_optional(&mut *state.db.acquire().await?)
		.await?
		.ok_or(RagError::CannotRetry)?;
		state.rag_ingest_notify.notify_one();
		Ok(updated)
	}

	/// Per-document "Limit access" targets in the shared assignment shape; empty means inheriting the knowledge base.
	///
	/// # Errors
	///
	/// Returns `NotManaged`, `Forbidden`, `NotFound` or a database error.
	pub async fn assignments(pool: &PgPool, actor: &KnowledgeActor, base: &KnowledgeBase, document_id: &Uuid) -> Result<AssignmentsResponse, RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		if !actor.can_edit(base) {
			return Err(RagError::Forbidden);
		}
		let doc = Self::find(pool, &base.id, document_id).await?.ok_or(RagError::NotFound)?;
		let targets = decode_access_targets(pool, doc.access.as_deref().unwrap_or(&[])).await?;
		Ok(AssignmentsResponse {
			kinds: Self::ACCESS_KINDS.to_vec(),
			single: false,
			roles: Vec::new(),
			targets,
		})
	}

	/// Replaces a document's access list on `rag_documents`, `rag_chunks` and the managed pgvector store.
	///
	/// An empty target list resets `access` to `NULL` so the document inherits the knowledge base's sharing again.
	///
	/// # Errors
	///
	/// Returns `NotManaged`, `Forbidden`, `NotFound`, `InvalidAssignment`, `Store` or a database error.
	pub async fn replace_assignments(
		state: &JobState,
		actor: &KnowledgeActor,
		base: &KnowledgeBase,
		document_id: &Uuid,
		request: &ReplaceAssignmentsRequest,
	) -> Result<(), RagError> {
		if !base.is_managed() {
			return Err(RagError::NotManaged);
		}
		if !actor.can_edit(base) {
			return Err(RagError::Forbidden);
		}
		if !request.is_valid_for(&Self::ACCESS_KINDS) {
			return Err(RagError::InvalidAssignment);
		}
		let users = request.ids(AssignmentKind::User);
		let teams = request.ids(AssignmentKind::Team);
		let roles = request.ids(AssignmentKind::Role);
		validate_targets_exist(&state.db, &users, &teams, &roles).await?;
		let labels = encode_access_labels(&users, &teams, &roles);
		let stored: Option<&[String]> = (!labels.is_empty()).then_some(labels.as_slice());

		let mut tx = state.db.begin().await?;
		let filename: Option<String> = sqlx::query_scalar(
			"UPDATE rag_documents SET access = $3, updated_at = NOW()
			 WHERE id = $1 AND knowledge_base_id = $2 AND status <> 'deleting'
			 RETURNING filename",
		)
		.bind(document_id)
		.bind(base.id)
		.bind(stored)
		.fetch_optional(&mut *tx)
		.await?;
		let Some(filename) = filename else {
			return Err(RagError::NotFound);
		};
		sqlx::query("UPDATE rag_chunks SET access = $2 WHERE document_id = $1")
			.bind(document_id)
			.bind(stored)
			.execute(&mut *tx)
			.await?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_document_access_updated",
				actor_id: &actor.user_id,
				target_type: AUDIT_TARGET,
				target_id: document_id,
				metadata: serde_json::json!({ "knowledge_base_id": base.id, "filename": filename, "restricted": stored.is_some(), "labels": labels }),
			},
		)
		.await?;
		tx.commit().await?;

		let connection = RagConnection::find(&state.db, &base.connection_id).await?.ok_or(RagError::NotFound)?;
		let env_url = RagConfig::get(&state.db).await?.env_url_for(&connection);
		let store = VectorStore::connect(&connection, &state.db, env_url.as_deref()).await?;
		store.update_managed_access(&connection.id, document_id, stored).await?;
		Ok(())
	}

	/// Public DTO for the Documents tab.
	#[must_use]
	pub fn to_response(&self) -> RagDocumentResponse {
		RagDocumentResponse {
			id: self.id,
			knowledge_base_id: self.knowledge_base_id,
			filename: self.filename.clone(),
			mime_type: self.mime_type.clone(),
			size_bytes: self.size_bytes,
			status: self.status,
			error: self.error.clone(),
			page_count: self.page_count,
			chunk_count: self.chunk_count,
			token_count: self.token_count,
			access_restricted: self.access.is_some(),
			can_retry: self.can_retry,
			created_at: self.created_at,
			updated_at: self.updated_at,
		}
	}
}

fn sanitize_filename(raw: &str) -> Result<&str, RagError> {
	let name = raw.trim();
	if name.is_empty() || name.chars().count() > 255 || name.contains(['/', '\\', '\0']) {
		return Err(RagError::InvalidFilename);
	}
	Ok(name)
}

async fn check_quotas(tx: &mut PgConnection, config: &RagConfig, user_id: &Uuid, upload_bytes: usize) -> Result<(), RagError> {
	let recent: i64 =
		sqlx::query_scalar("SELECT COUNT(*)::bigint FROM audit_logs WHERE actor_id = $1 AND event = 'rag_document_uploaded' AND created_at > NOW() - INTERVAL '1 hour'")
			.bind(user_id)
			.fetch_one(&mut *tx)
			.await?;
	if recent >= i64::from(config.max_uploads_per_hour) {
		return Err(RagError::RateLimited);
	}
	if let Some(quota_mb) = config.storage_quota_mb {
		let quota_bytes = i64::from(quota_mb).saturating_mul(1024 * 1024);
		let used: i64 = sqlx::query_scalar(
			"SELECT COALESCE(SUM(GREATEST(COALESCE(octet_length(text), 0), COALESCE(octet_length(source_bytes), 0))), 0)::bigint
			 FROM rag_documents WHERE uploaded_by = $1 AND status <> 'deleting'",
		)
		.bind(user_id)
		.fetch_one(&mut *tx)
		.await?;
		if used.saturating_add(i64::try_from(upload_bytes).unwrap_or(i64::MAX)) > quota_bytes {
			return Err(RagError::StorageQuotaExceeded);
		}
	}
	Ok(())
}

async fn validate_targets_exist(pool: &PgPool, users: &[Uuid], teams: &[Uuid], roles: &[Uuid]) -> Result<(), RagError> {
	if !users.is_empty() {
		let found: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users WHERE id = ANY($1)").bind(users).fetch_one(pool).await?;
		if usize::try_from(found).unwrap_or(0) != users.len() {
			return Err(RagError::InvalidAssignment);
		}
	}
	if !teams.is_empty() {
		let found: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM teams WHERE id = ANY($1)").bind(teams).fetch_one(pool).await?;
		if usize::try_from(found).unwrap_or(0) != teams.len() {
			return Err(RagError::InvalidAssignment);
		}
	}
	if !roles.is_empty() {
		let found: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM roles WHERE id = ANY($1)").bind(roles).fetch_one(pool).await?;
		if usize::try_from(found).unwrap_or(0) != roles.len() {
			return Err(RagError::InvalidAssignment);
		}
	}
	Ok(())
}

/// Encodes user, team and role IDs into canonical `user:<id>`, `team:<id>`, `role:<id>` labels.
#[must_use]
pub fn encode_access_labels(users: &[Uuid], teams: &[Uuid], roles: &[Uuid]) -> Vec<String> {
	let mut labels = Vec::with_capacity(users.len() + teams.len() + roles.len());
	for id in teams {
		labels.push(format!("team:{id}"));
	}
	for id in roles {
		labels.push(format!("role:{id}"));
	}
	for id in users {
		labels.push(format!("user:{id}"));
	}
	labels.sort();
	labels.dedup();
	labels
}

async fn decode_access_targets(pool: &PgPool, labels: &[String]) -> Result<Vec<AssignmentTarget>, sqlx::Error> {
	let mut team_ids = Vec::new();
	let mut role_ids = Vec::new();
	let mut user_ids = Vec::new();
	for label in labels {
		if let Some(id) = label.strip_prefix("team:").and_then(|value| Uuid::parse_str(value).ok()) {
			team_ids.push(id);
		} else if let Some(id) = label.strip_prefix("role:").and_then(|value| Uuid::parse_str(value).ok()) {
			role_ids.push(id);
		} else if let Some(id) = label.strip_prefix("user:").and_then(|value| Uuid::parse_str(value).ok()) {
			user_ids.push(id);
		}
	}
	let rows = sqlx::query_as::<_, (String, Uuid, String)>(
		"SELECT 'team', id, name::text FROM teams WHERE id = ANY($1)
		 UNION ALL SELECT 'role', id, name::text FROM roles WHERE id = ANY($2)
		 UNION ALL SELECT 'user', id, username::text FROM users WHERE id = ANY($3)
		 ORDER BY 1, 3",
	)
	.bind(&team_ids)
	.bind(&role_ids)
	.bind(&user_ids)
	.fetch_all(pool)
	.await?;
	Ok(rows
		.into_iter()
		.map(|(kind, id, label)| AssignmentTarget {
			kind: AssignmentKind::from_stored(&kind),
			id: Some(id),
			label,
			inherited: false,
			owner: false,
			role: None,
		})
		.collect())
}
