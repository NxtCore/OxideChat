//! Knowledge bases (managed pgvector slices and external read-only views): scopes, sharing, hybrid search and deletion.

use super::{
	ConnectionKind, ConnectionMode, EmbeddingSettings, ExternalView, KnowledgeBaseConnection, KnowledgeBaseResponse, KnowledgeHit, KnowledgeScope, KnowledgeScopeTeam,
	KnowledgeScopesResponse, KnowledgeSearchResponse, ManagedSearch, RagConfig, RagConnection, RagError, SaveKnowledgeBaseRequest, StoreError, VectorStore,
	check_search_language, embed, probe_embedding,
};
use crate::logging::AuditLog;
use crate::types::consts::{ADMIN_RAG_EDIT, ADMIN_RAG_VIEW, SETTINGS_KNOWLEDGE_EDIT};
use crate::types::{AssignmentKind, AssignmentRole, AssignmentTarget, AssignmentsResponse, AuditEntry, JobState, ReplaceAssignmentsRequest, User};
use sqlx::types::Json;
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

const AUDIT_TARGET: &str = "knowledge_base";

/// Hits a test search returns.
pub const TEST_SEARCH_HITS: u32 = 8;

const FTS_TIMEOUT_MS: &str = "1500ms";
const MANAGED_RRF_K: f64 = 60.0;

macro_rules! columns {
	() => {
		"kb.id, kb.name, kb.description, kb.connection_id, kb.scope, kb.owner_id, kb.team_id,
		 t.name::text AS team_name, kb.search_language, kb.view, kb.shared_with_everyone,
		 c.name AS connection_name, c.kind AS connection_kind, c.mode AS connection_mode,
		 (SELECT COUNT(*)::bigint FROM rag_documents d WHERE d.knowledge_base_id = kb.id AND d.status <> 'deleting') AS document_count,
		 EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND s.role = 'editor' AND (
			s.user_id = $1
			OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
		 )) AS editor,
		 COALESCE(kb.owner_id = $1
			OR kb.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR kb.shared_with_everyone
			OR EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND (
				s.user_id = $1
				OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
				OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
			)), false) AS usable
		 FROM knowledge_bases kb
		 JOIN rag_connections c ON c.id = kb.connection_id
		 LEFT JOIN teams t ON t.id = kb.team_id"
	};
}

macro_rules! visible {
	() => {
		"(kb.deleted_at IS NULL AND (
			kb.owner_id = $1
			OR kb.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR kb.shared_with_everyone
			OR EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND (
				s.user_id = $1
				OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
				OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
			))
		))"
	};
}

/// A knowledge base as loaded for one viewer.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeBase {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub connection_id: Uuid,
	pub scope: KnowledgeScope,
	pub owner_id: Option<Uuid>,
	pub team_id: Option<Uuid>,
	pub team_name: Option<String>,
	pub search_language: String,
	pub view: Option<Json<ExternalView>>,
	pub shared_with_everyone: bool,
	pub connection_name: String,
	pub connection_kind: ConnectionKind,
	pub connection_mode: ConnectionMode,
	pub document_count: i64,
	pub editor: bool,
	/// Shared with or owned by the viewer, so they may search it; knowledge admins see others too.
	pub usable: bool,
}

/// Who is acting on knowledge bases.
#[derive(Debug, Clone)]
pub struct KnowledgeActor {
	pub user_id: Uuid,
	pub manage: bool,
	pub view_all: bool,
	pub create_personal: bool,
	pub admin_team_ids: Vec<Uuid>,
}

impl KnowledgeActor {
	/// Builds an actor from the user's permissions and managed teams.
	///
	/// # Errors
	///
	/// Returns a database error when team admin memberships cannot be loaded.
	pub async fn for_user(pool: &PgPool, user_id: Uuid, permissions: &[String]) -> Result<Self, sqlx::Error> {
		let holds = |permission: &str| permissions.iter().any(|held| User::permission_matches(held, permission));
		let manage = holds(ADMIN_RAG_EDIT);
		let admin_team_ids: Vec<Uuid> = sqlx::query_scalar("SELECT team_id FROM team_members WHERE user_id = $1 AND role = 'admin'")
			.bind(user_id)
			.fetch_all(pool)
			.await?;
		Ok(Self {
			user_id,
			manage,
			view_all: manage || holds(ADMIN_RAG_VIEW),
			create_personal: manage || holds(SETTINGS_KNOWLEDGE_EDIT),
			admin_team_ids,
		})
	}

	/// Convenience constructor for tests and simple permission cases.
	#[cfg(test)]
	#[must_use]
	pub fn basic(user_id: Uuid, manage: bool, view_all: bool) -> Self {
		Self {
			user_id,
			manage,
			view_all: manage || view_all,
			create_personal: manage,
			admin_team_ids: Vec::new(),
		}
	}

	/// Whether the actor owns this managed knowledge base (personal owner or admin of its owning team).
	#[must_use]
	pub fn owns(&self, kb: &KnowledgeBase) -> bool {
		if !kb.is_managed() {
			return false;
		}
		match kb.scope {
			KnowledgeScope::Personal => kb.owner_id == Some(self.user_id),
			KnowledgeScope::Team => kb.team_id.is_some_and(|team| self.admin_team_ids.contains(&team)),
			KnowledgeScope::Installation => false,
		}
	}

	/// Whether the actor may manage (delete, share) `knowledge_base`.
	#[must_use]
	pub fn can_manage(&self, kb: &KnowledgeBase) -> bool {
		self.manage || self.owns(kb)
	}

	/// Whether the actor may change name, description, search language and upload/edit managed documents.
	#[must_use]
	pub fn can_edit(&self, kb: &KnowledgeBase) -> bool {
		self.can_manage(kb) || kb.editor
	}

	/// Whether the actor may run a test search: it is shared with or owned by them, or they manage it.
	#[must_use]
	pub fn can_search(&self, kb: &KnowledgeBase) -> bool {
		self.can_manage(kb) || kb.usable
	}

	/// Target kinds the actor may share `kb` with.
	#[must_use]
	pub fn share_kinds(&self, kb: &KnowledgeBase) -> Vec<AssignmentKind> {
		if !self.can_manage(kb) {
			return Vec::new();
		}
		if self.manage {
			KnowledgeBase::ASSIGNMENT_KINDS.to_vec()
		} else if !self.admin_team_ids.is_empty() {
			vec![AssignmentKind::Team]
		} else {
			Vec::new()
		}
	}
}

/// One hit from a managed knowledge base's hybrid search, keeping its `document_id` for per-document capping.
#[derive(Debug, Clone)]
pub struct ManagedHit {
	pub document_id: Uuid,
	pub hit: KnowledgeHit,
}

/// Inputs for hybrid (vector + full-text) search of one managed knowledge base.
pub struct ManagedHybridQuery<'a> {
	pub base: &'a KnowledgeBase,
	pub connection: &'a RagConnection,
	pub query: &'a str,
	pub vector: &'a [f32],
	pub limit: u32,
	pub labels: &'a [String],
}

struct UpdateContext<'a> {
	actor: &'a KnowledgeActor,
	request: &'a SaveKnowledgeBaseRequest,
	targets: Option<&'a super::ModelTargets>,
	next_language: Option<&'static str>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChunkRow {
	id: Uuid,
	document_id: Uuid,
	filename: String,
	heading_path: Option<String>,
	page: Option<i32>,
	content: String,
}

async fn audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, id: &Uuid, metadata: serde_json::Value) -> Result<(), sqlx::Error> {
	AuditLog::write(
		tx,
		AuditEntry {
			event,
			actor_id,
			target_type: AUDIT_TARGET,
			target_id: id,
			metadata,
		},
	)
	.await
}

fn view_metadata(name: &str, view: &ExternalView) -> serde_json::Value {
	serde_json::json!({ "name": name, "connection_id": view.connection_id, "collection": view.collection, "filter": view.filter, "embedding_model_id": view.embedding.model_id })
}

/// Checks `view` against its store: a read-only connection, an existing vector, filter values the store can match
/// (Qdrant matches no decimal numbers) and an embedding model whose vectors have the collection's size.
///
/// Sends one test embedding billed to `user_id`. Returns the view with Qdrant's own distance.
///
/// # Errors
///
/// Returns why the view cannot be searched.
pub async fn check_view(state: &JobState, user_id: &Uuid, view: &ExternalView) -> Result<ExternalView, RagError> {
	let connection = RagConnection::find(&state.db, &view.connection_id).await?.ok_or(RagError::NotFound)?;
	if connection.mode != ConnectionMode::ReadOnly {
		return Err(RagError::NotReadOnly);
	}
	if connection.kind == ConnectionKind::Qdrant && view.filter.iter().any(|filter| filter.value.as_number().is_some_and(|number| !number.is_i64() && !number.is_u64())) {
		return Err(RagError::InvalidFilter);
	}
	let store = VectorStore::connect(&connection, &state.db, None).await?;
	let info = store.describe(&view.collection, &view.filter).await?;
	let vector = info.vectors.iter().find(|vector| vector.name == view.vector_name).ok_or(RagError::VectorNotFound)?;
	let size = probe_embedding(state, user_id, &view.embedding).await?;
	if vector.size.is_some_and(|expected| expected != size) {
		return Err(RagError::CollectionDimensionsMismatch);
	}
	Ok(ExternalView {
		distance: vector.distance.unwrap_or(view.distance),
		..view.clone()
	})
}

/// Runs hybrid search (managed pgvector similarity + time-limited Postgres FTS, re-checked in Postgres and merged with RRF).
///
/// # Errors
///
/// Returns `StoreError::Reindexing` while the connection is re-indexing, or another `StoreError` when the vector store fails.
pub async fn hybrid_search_managed(state: &JobState, params: &ManagedHybridQuery<'_>) -> Result<Vec<ManagedHit>, StoreError> {
	let reindexing = params.connection.is_reindexing(&state.db).await.map_err(|_| StoreError::Unreachable)?;
	if reindexing {
		return Err(StoreError::Reindexing);
	}
	let config = RagConfig::get(&state.db).await.map_err(|_| StoreError::Unreachable)?;
	let env_url = config.env_url_for(params.connection);
	let store = VectorStore::connect(params.connection, &state.db, env_url.as_deref()).await?;
	let language = check_search_language(&params.base.search_language).unwrap_or("simple");

	let managed_search = ManagedSearch {
		connection_id: &params.connection.id,
		generation: params.connection.index_generation,
		knowledge_base_id: &params.base.id,
		vector: params.vector,
		limit: params.limit,
		labels: params.labels,
	};
	let vector_future = store.search_managed(&managed_search);
	let fts_future = fts_search_managed(&state.db, params, language);
	let (vector_result, fts_rows) = tokio::join!(vector_future, fts_future);
	let vector_ids = vector_result?;
	let vector_rows = verify_managed_chunks(&state.db, &params.base.id, &vector_ids, params.labels)
		.await
		.map_err(|_| StoreError::Unreachable)?;
	Ok(merge_managed_hits(vector_rows, fts_rows, params.limit as usize))
}

async fn fts_search_managed(pool: &PgPool, params: &ManagedHybridQuery<'_>, language: &'static str) -> Vec<ChunkRow> {
	let Ok(mut tx) = pool.begin().await else {
		return Vec::new();
	};
	if sqlx::query("SELECT set_config('statement_timeout', $1, true)")
		.bind(FTS_TIMEOUT_MS)
		.execute(&mut *tx)
		.await
		.is_err()
	{
		return Vec::new();
	}
	match sqlx::query_as::<_, ChunkRow>(
		"SELECT c.id, c.document_id, d.filename, c.heading_path, c.page, c.content
		 FROM rag_chunks c
		 JOIN rag_documents d ON d.id = c.document_id
		 WHERE c.knowledge_base_id = $1
		   AND d.status = 'ready'
		   AND (c.access IS NULL OR c.access && $4::text[])
		   AND c.search @@ websearch_to_tsquery($2::regconfig, $3)
		 ORDER BY ts_rank_cd(c.search, websearch_to_tsquery($2::regconfig, $3)) DESC, c.id
		 LIMIT $5",
	)
	.bind(params.base.id)
	.bind(language)
	.bind(params.query)
	.bind(params.labels)
	.bind(i64::from(params.limit))
	.fetch_all(&mut *tx)
	.await
	{
		Ok(rows) => rows,
		Err(error) => {
			tracing::warn!(knowledge_base = %params.base.id, %error, "managed full-text search skipped");
			Vec::new()
		}
	}
}

async fn verify_managed_chunks(pool: &PgPool, kb_id: &Uuid, ranked: &[(Uuid, f64)], labels: &[String]) -> Result<Vec<ChunkRow>, sqlx::Error> {
	if ranked.is_empty() {
		return Ok(Vec::new());
	}
	let ids: Vec<Uuid> = ranked.iter().map(|(id, _)| *id).collect();
	let rows: Vec<ChunkRow> = sqlx::query_as(
		"SELECT c.id, c.document_id, d.filename, c.heading_path, c.page, c.content
		 FROM rag_chunks c
		 JOIN rag_documents d ON d.id = c.document_id
		 WHERE c.id = ANY($1)
		   AND c.knowledge_base_id = $2
		   AND d.status = 'ready'
		   AND (c.access IS NULL OR c.access && $3::text[])",
	)
	.bind(&ids)
	.bind(kb_id)
	.bind(labels)
	.fetch_all(pool)
	.await?;
	let mut by_id: HashMap<Uuid, ChunkRow> = rows.into_iter().map(|row| (row.id, row)).collect();
	Ok(ids.into_iter().filter_map(|id| by_id.remove(&id)).collect())
}

#[cfg(test)]
pub async fn test_fts_chunk_ids(pool: &PgPool, base: &KnowledgeBase, connection: &RagConnection, query: &str, labels: &[String]) -> Vec<Uuid> {
	let language = check_search_language(&base.search_language).unwrap_or("simple");
	let params = ManagedHybridQuery {
		base,
		connection,
		query,
		vector: &[],
		limit: 20,
		labels,
	};
	fts_search_managed(pool, &params, language)
		.await
		.into_iter()
		.map(|row| row.id)
		.collect()
}

#[cfg(test)]
pub async fn test_verified_chunk_ids(pool: &PgPool, kb_id: &Uuid, ids: &[Uuid], labels: &[String]) -> Result<Vec<Uuid>, sqlx::Error> {
	let ranked: Vec<(Uuid, f64)> = ids.iter().map(|id| (*id, 1.0)).collect();
	Ok(verify_managed_chunks(pool, kb_id, &ranked, labels)
		.await?
		.into_iter()
		.map(|row| row.id)
		.collect())
}

fn merge_managed_hits(vector_rows: Vec<ChunkRow>, fts_rows: Vec<ChunkRow>, limit: usize) -> Vec<ManagedHit> {
	let mut scores: HashMap<Uuid, (f64, ChunkRow)> = HashMap::new();
	for list in [vector_rows, fts_rows] {
		for (rank, row) in list.into_iter().enumerate() {
			let contribution = 1.0 / (MANAGED_RRF_K + rank as f64 + 1.0);
			scores
				.entry(row.id)
				.and_modify(|(total, _)| *total += contribution)
				.or_insert((contribution, row));
		}
	}
	let mut ranked: Vec<(Uuid, (f64, ChunkRow))> = scores.into_iter().collect();
	ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
	let mut per_doc: HashMap<Uuid, usize> = HashMap::new();
	let mut hits = Vec::with_capacity(limit.min(ranked.len()));
	for (_, (score, row)) in ranked {
		let count = per_doc.entry(row.document_id).or_insert(0);
		*count += 1;
		if *count > 3 {
			continue;
		}
		hits.push(ManagedHit {
			document_id: row.document_id,
			hit: KnowledgeHit {
				text: row.content,
				title: Some(row.filename),
				url: None,
				section: row.heading_path,
				page: row.page,
				score,
			},
		});
		if hits.len() >= limit {
			break;
		}
	}
	hits
}

impl KnowledgeBase {
	/// What a knowledge base can be shared with.
	pub const ASSIGNMENT_KINDS: [AssignmentKind; 4] = [AssignmentKind::Everyone, AssignmentKind::Team, AssignmentKind::Role, AssignmentKind::User];

	/// True when this knowledge base lives in a managed pgvector connection.
	#[must_use]
	pub const fn is_managed(&self) -> bool {
		matches!(self.connection_mode, ConnectionMode::Managed)
	}

	/// The external view if this is an external knowledge base.
	#[must_use]
	pub fn external_view(&self) -> Option<&ExternalView> {
		self.view.as_ref().map(|view| &view.0)
	}

	/// Where `actor` may create knowledge bases and whether a default managed connection is ready.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn scopes(pool: &PgPool, actor: &KnowledgeActor) -> Result<KnowledgeScopesResponse, sqlx::Error> {
		let config = RagConfig::get(pool).await?;
		let managed_available = match config.default_connection_id {
			Some(id) => RagConnection::find(pool, &id)
				.await?
				.is_some_and(|conn| conn.mode == ConnectionMode::Managed && conn.embedding_model_id.is_some() && conn.vector_size.is_some()),
			None => false,
		};
		let teams: Vec<KnowledgeScopeTeam> = if actor.manage {
			sqlx::query_as("SELECT id, name::text FROM teams ORDER BY lower(name), id").fetch_all(pool).await?
		} else if !actor.admin_team_ids.is_empty() {
			sqlx::query_as("SELECT id, name::text FROM teams WHERE id = ANY($1) ORDER BY lower(name), id")
				.bind(&actor.admin_team_ids)
				.fetch_all(pool)
				.await?
		} else {
			Vec::new()
		};
		Ok(KnowledgeScopesResponse {
			managed_available,
			personal: actor.create_personal,
			teams,
			installation: actor.manage,
			external: actor.manage,
		})
	}

	/// Knowledge bases the actor may see, by name: every non-deleted one for knowledge admins, otherwise those owned or shared with them.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool, actor: &KnowledgeActor) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			columns!(),
			" WHERE kb.deleted_at IS NULL AND ($2 OR ",
			visible!(),
			") ORDER BY lower(kb.name), kb.id"
		))
		.bind(actor.user_id)
		.bind(actor.view_all)
		.fetch_all(pool)
		.await
	}

	/// One non-deleted knowledge base the actor may see.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, actor: &KnowledgeActor, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			columns!(),
			" WHERE kb.id = $3 AND kb.deleted_at IS NULL AND ($2 OR ",
			visible!(),
			")"
		))
		.bind(actor.user_id)
		.bind(actor.view_all)
		.bind(id)
		.fetch_optional(pool)
		.await
	}

	/// The knowledge bases among `ids` that `user_id` may search right now; the IDs come from chats and presets, never trusted as given.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn accessible(executor: impl PgExecutor<'_>, user_id: &Uuid, ids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
		if ids.is_empty() {
			return Ok(Vec::new());
		}
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			columns!(),
			" WHERE kb.id = ANY($2) AND ",
			visible!(),
			" ORDER BY lower(kb.name), kb.id"
		))
		.bind(user_id)
		.bind(ids)
		.fetch_all(executor)
		.await
	}

	/// The knowledge bases attached to a chat, whoever may still search them.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn chat_ids(pool: &PgPool, chat_id: &Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
		sqlx::query_scalar("SELECT knowledge_base_id FROM chat_knowledge_bases WHERE chat_id = $1 ORDER BY knowledge_base_id")
			.bind(chat_id)
			.fetch_all(pool)
			.await
	}

	/// The knowledge bases attached to a preset.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn preset_ids(executor: impl PgExecutor<'_>, preset_id: &Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
		sqlx::query_scalar("SELECT knowledge_base_id FROM preset_knowledge_bases WHERE preset_id = $1 ORDER BY knowledge_base_id")
			.bind(preset_id)
			.fetch_all(executor)
			.await
	}

	/// Makes the knowledge bases among `ids` that `user_id` may search the only ones attached to the chat; returns them.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn attach_to_chat(pool: &PgPool, user_id: &Uuid, chat_id: &Uuid, ids: &[Uuid]) -> Result<Vec<Uuid>, sqlx::Error> {
		let allowed: Vec<Uuid> = Self::accessible(pool, user_id, ids).await?.into_iter().map(|base| base.id).collect();
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM chat_knowledge_bases WHERE chat_id = $1")
			.bind(chat_id)
			.execute(&mut *tx)
			.await?;
		sqlx::query("INSERT INTO chat_knowledge_bases (chat_id, knowledge_base_id) SELECT $1, unnest($2::uuid[])")
			.bind(chat_id)
			.bind(&allowed)
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		Ok(allowed)
	}

	/// Makes `ids` the knowledge bases attached to a preset in the caller's transaction; the caller must be allowed to see each of them.
	///
	/// # Errors
	///
	/// Returns a database error when a statement fails.
	pub async fn attach_to_preset(tx: &mut Transaction<'_, Postgres>, preset_id: &Uuid, ids: &[Uuid]) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM preset_knowledge_bases WHERE preset_id = $1")
			.bind(preset_id)
			.execute(&mut **tx)
			.await?;
		sqlx::query("INSERT INTO preset_knowledge_bases (preset_id, knowledge_base_id) SELECT $1, unnest($2::uuid[])")
			.bind(preset_id)
			.bind(ids)
			.execute(&mut **tx)
			.await?;
		Ok(())
	}

	/// Stores a new external knowledge base shared with nobody; the view must have been checked with [`check_view`].
	///
	/// # Errors
	///
	/// Returns `Forbidden` unless the actor manages knowledge, `InvalidCollection` without a view, or a database error.
	pub async fn create(pool: &PgPool, actor: &KnowledgeActor, request: &SaveKnowledgeBaseRequest, view: &ExternalView) -> Result<Uuid, RagError> {
		if !actor.manage {
			return Err(RagError::Forbidden);
		}
		request.validate()?;
		let mut tx = pool.begin().await?;
		let id: Uuid = sqlx::query_scalar(
			"INSERT INTO knowledge_bases (name, description, connection_id, scope, view, created_by)
			 VALUES ($1, $2, $3, 'installation', $4, $5) RETURNING id",
		)
		.bind(request.name.trim())
		.bind(request.description())
		.bind(view.connection_id)
		.bind(Json(view))
		.bind(actor.user_id)
		.fetch_one(&mut *tx)
		.await?;
		audit(&mut tx, "knowledge_base_created", &actor.user_id, &id, view_metadata(request.name.trim(), view)).await?;
		tx.commit().await?;
		Ok(id)
	}

	/// Creates a managed knowledge base on the default managed connection; shared with nobody beyond its owner/team.
	///
	/// # Errors
	///
	/// Returns `Forbidden`, `InvalidScope`, `NoDefaultConnection`, `ConnectionNotReady` or a database error.
	pub async fn create_managed(pool: &PgPool, actor: &KnowledgeActor, request: &SaveKnowledgeBaseRequest) -> Result<Uuid, RagError> {
		request.validate()?;
		let scope = request.scope.unwrap_or(KnowledgeScope::Personal);
		let (owner_id, team_id) = match scope {
			KnowledgeScope::Personal => {
				if !actor.create_personal {
					return Err(RagError::Forbidden);
				}
				if request.team_id.is_some() {
					return Err(RagError::InvalidScope);
				}
				(Some(actor.user_id), None)
			}
			KnowledgeScope::Team => {
				let team_id = request.team_id.ok_or(RagError::InvalidScope)?;
				if !actor.manage && !actor.admin_team_ids.contains(&team_id) {
					return Err(RagError::Forbidden);
				}
				(None, Some(team_id))
			}
			KnowledgeScope::Installation => {
				if !actor.manage {
					return Err(RagError::Forbidden);
				}
				if request.team_id.is_some() {
					return Err(RagError::InvalidScope);
				}
				(None, None)
			}
		};
		let language = request.search_language.as_deref().map(check_search_language).transpose()?.unwrap_or("simple");
		let config = RagConfig::get(pool).await?;
		let connection_id = config.default_connection_id.ok_or(RagError::NoDefaultConnection)?;
		let connection = RagConnection::find(pool, &connection_id).await?.ok_or(RagError::NoDefaultConnection)?;
		if connection.mode != ConnectionMode::Managed {
			return Err(RagError::NoDefaultConnection);
		}
		if connection.embedding_model_id.is_none() || connection.vector_size.is_none() {
			return Err(RagError::ConnectionNotReady);
		}

		let mut tx = pool.begin().await?;
		let id: Uuid = sqlx::query_scalar(
			"INSERT INTO knowledge_bases (name, description, connection_id, scope, owner_id, team_id, search_language, view, created_by)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8) RETURNING id",
		)
		.bind(request.name.trim())
		.bind(request.description())
		.bind(connection.id)
		.bind(scope)
		.bind(owner_id)
		.bind(team_id)
		.bind(language)
		.bind(actor.user_id)
		.fetch_one(&mut *tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::InvalidScope))?;
		audit(
			&mut tx,
			"knowledge_base_created",
			&actor.user_id,
			&id,
			serde_json::json!({ "name": request.name.trim(), "mode": "managed", "scope": scope, "team_id": team_id, "connection_id": connection.id, "search_language": language }),
		)
		.await?;
		tx.commit().await?;
		Ok(id)
	}

	/// Changes name, description, search language (managed) and, with `view` or `request.model_access`, the source and model access.
	///
	/// # Errors
	///
	/// Returns `Forbidden`, `EmbeddingModelNotAllowed`, `InvalidModelAccess`, `UpstreamNeedsProvider`, `NotFound` or a database error.
	pub async fn update(&self, pool: &PgPool, actor: &KnowledgeActor, request: &SaveKnowledgeBaseRequest, view: Option<&ExternalView>) -> Result<(), RagError> {
		if !actor.can_edit(self) || ((view.is_some() || request.model_access.is_some()) && !actor.manage) {
			return Err(RagError::Forbidden);
		}
		if self.is_managed() && view.is_some() {
			return Err(RagError::NotReadOnly);
		}
		request.validate()?;
		let targets = request.model_access.as_ref().map(super::ModelTargets::normalized).transpose()?;
		if let Some(targets) = &targets {
			super::check_upstream_needs_provider(pool, targets).await?;
		}
		let next_language = request.search_language.as_deref().map(check_search_language).transpose()?;

		let mut tx = pool.begin().await?;
		let stored: Option<(Option<Json<ExternalView>>, String, Option<Uuid>)> = sqlx::query_as(
			"SELECT kb.view, kb.search_language, c.embedding_model_id
			 FROM knowledge_bases kb JOIN rag_connections c ON c.id = kb.connection_id
			 WHERE kb.id = $1 AND kb.deleted_at IS NULL FOR UPDATE OF kb",
		)
		.bind(self.id)
		.fetch_optional(&mut *tx)
		.await?;
		let (stored_view, stored_lang, conn_embedding) = stored.ok_or(RagError::NotFound)?;

		let ctx = UpdateContext {
			actor,
			request,
			targets: targets.as_ref(),
			next_language,
		};
		if self.is_managed() {
			self.update_managed(&mut tx, &ctx, &stored_lang, conn_embedding).await?;
		} else {
			let stored_view = stored_view.ok_or(RagError::NotFound)?.0;
			self.update_external(&mut tx, &ctx, view.unwrap_or(&stored_view), &stored_view).await?;
		}

		if let Some(targets) = &targets {
			self.store_model_access(&mut tx, &actor.user_id, targets).await?;
		}
		tx.commit().await?;
		Ok(())
	}

	async fn update_managed(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		ctx: &UpdateContext<'_>,
		stored_lang: &str,
		conn_embedding: Option<Uuid>,
	) -> Result<(), RagError> {
		if let Some(targets) = ctx.targets
			&& let Some(model_id) = conn_embedding
		{
			super::check_embedding_allowed(&mut **tx, targets, &model_id).await?;
		}
		let language = ctx.next_language.unwrap_or(stored_lang);
		sqlx::query("UPDATE knowledge_bases SET name = $2, description = $3, search_language = $4, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(ctx.request.name.trim())
			.bind(ctx.request.description())
			.bind(language)
			.execute(&mut **tx)
			.await?;
		if language != stored_lang {
			sqlx::query("UPDATE rag_chunks SET search = to_tsvector($2::regconfig, content) WHERE knowledge_base_id = $1")
				.bind(self.id)
				.bind(language)
				.execute(&mut **tx)
				.await?;
		}
		audit(
			tx,
			"knowledge_base_updated",
			&ctx.actor.user_id,
			&self.id,
			serde_json::json!({ "name": ctx.request.name.trim(), "mode": "managed", "search_language": language }),
		)
		.await?;
		Ok(())
	}

	async fn update_external(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		ctx: &UpdateContext<'_>,
		view: &ExternalView,
		stored_view: &ExternalView,
	) -> Result<(), RagError> {
		if ctx.targets.is_some() || view != stored_view {
			let current = match ctx.targets {
				Some(targets) => targets.clone(),
				None => super::ModelTargets::load(&mut **tx, &[self.id]).await?.remove(&self.id).unwrap_or_default(),
			};
			super::check_embedding_allowed(&mut **tx, &current, &view.embedding.model_id).await?;
		}
		sqlx::query("UPDATE knowledge_bases SET name = $2, description = $3, connection_id = $4, view = $5, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(ctx.request.name.trim())
			.bind(ctx.request.description())
			.bind(view.connection_id)
			.bind(Json(view))
			.execute(&mut **tx)
			.await?;
		audit(tx, "knowledge_base_updated", &ctx.actor.user_id, &self.id, view_metadata(ctx.request.name.trim(), view)).await?;
		Ok(())
	}

	/// Deletes the knowledge base; managed bases with documents are soft-deleted and their documents marked `deleting` so the worker purges vectors first.
	///
	/// Returns `true` when documents were queued for vector deletion so callers can wake the ingest worker.
	///
	/// # Errors
	///
	/// Returns `Forbidden`, `NotFound` or a database error.
	pub async fn delete(&self, pool: &PgPool, actor: &KnowledgeActor) -> Result<bool, RagError> {
		if !actor.can_manage(self) {
			return Err(RagError::Forbidden);
		}
		let mut tx = pool.begin().await?;
		let has_docs: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM rag_documents WHERE knowledge_base_id = $1)")
			.bind(self.id)
			.fetch_one(&mut *tx)
			.await?;
		if has_docs {
			let updated = sqlx::query("UPDATE knowledge_bases SET deleted_at = NOW(), shared_with_everyone = FALSE, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
				.bind(self.id)
				.execute(&mut *tx)
				.await?;
			if updated.rows_affected() == 0 {
				return Err(RagError::NotFound);
			}
			sqlx::query("DELETE FROM knowledge_base_shares WHERE knowledge_base_id = $1").bind(self.id).execute(&mut *tx).await?;
			sqlx::query("DELETE FROM chat_knowledge_bases WHERE knowledge_base_id = $1").bind(self.id).execute(&mut *tx).await?;
			sqlx::query("DELETE FROM preset_knowledge_bases WHERE knowledge_base_id = $1").bind(self.id).execute(&mut *tx).await?;
			sqlx::query("UPDATE rag_documents SET status = 'deleting', source_bytes = NULL, locked_at = NULL, updated_at = NOW() WHERE knowledge_base_id = $1")
				.bind(self.id)
				.execute(&mut *tx)
				.await?;
		} else {
			let deleted = sqlx::query("DELETE FROM knowledge_bases WHERE id = $1").bind(self.id).execute(&mut *tx).await?;
			if deleted.rows_affected() == 0 {
				return Err(RagError::NotFound);
			}
		}
		audit(&mut tx, "knowledge_base_deleted", &actor.user_id, &self.id, serde_json::json!({ "name": self.name })).await?;
		tx.commit().await?;
		Ok(has_docs)
	}

	/// Marks every personal knowledge base owned by `user_id` for vector-safe deletion inside the user-deletion transaction.
	///
	/// Embeddings count as personal data and are never orphaned by cascading rows away: bases with documents are soft-deleted
	/// and their documents marked `deleting` for the worker; empty personal bases are removed immediately.
	///
	/// # Errors
	///
	/// Returns a database error when a statement fails.
	pub async fn delete_user_personal(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM knowledge_bases WHERE scope = 'personal' AND owner_id = $1 AND deleted_at IS NULL")
			.bind(user_id)
			.fetch_all(&mut **tx)
			.await?;
		if ids.is_empty() {
			return Ok(false);
		}
		sqlx::query("DELETE FROM knowledge_base_shares WHERE knowledge_base_id = ANY($1)").bind(&ids).execute(&mut **tx).await?;
		sqlx::query("DELETE FROM chat_knowledge_bases WHERE knowledge_base_id = ANY($1)").bind(&ids).execute(&mut **tx).await?;
		sqlx::query("DELETE FROM preset_knowledge_bases WHERE knowledge_base_id = ANY($1)").bind(&ids).execute(&mut **tx).await?;
		let marked = sqlx::query(
			"UPDATE rag_documents SET status = 'deleting', source_bytes = NULL, locked_at = NULL, updated_at = NOW()
			 WHERE knowledge_base_id = ANY($1)",
		)
		.bind(&ids)
		.execute(&mut **tx)
		.await?;
		sqlx::query(
			"UPDATE knowledge_bases SET deleted_at = NOW(), owner_id = NULL, shared_with_everyone = FALSE, updated_at = NOW()
			 WHERE id = ANY($1) AND EXISTS (SELECT 1 FROM rag_documents d WHERE d.knowledge_base_id = knowledge_bases.id)",
		)
		.bind(&ids)
		.execute(&mut **tx)
		.await?;
		sqlx::query(
			"DELETE FROM knowledge_bases
			 WHERE id = ANY($1) AND NOT EXISTS (SELECT 1 FROM rag_documents d WHERE d.knowledge_base_id = knowledge_bases.id)",
		)
		.bind(&ids)
		.execute(&mut **tx)
		.await?;
		Ok(marked.rows_affected() > 0)
	}

	/// Who may use the knowledge base, in the shared assignment shape.
	///
	/// # Errors
	///
	/// Returns a database error when the shares cannot be loaded.
	pub async fn assignments(&self, pool: &PgPool, actor: &KnowledgeActor) -> Result<AssignmentsResponse, sqlx::Error> {
		let rows = sqlx::query_as::<_, (String, Uuid, String, String)>(
			"SELECT 'team', t.id, t.name::text, s.role FROM knowledge_base_shares s JOIN teams t ON t.id = s.team_id WHERE s.knowledge_base_id = $1
			 UNION ALL SELECT 'role', r.id, r.name::text, s.role FROM knowledge_base_shares s JOIN roles r ON r.id = s.role_id WHERE s.knowledge_base_id = $1
			 UNION ALL SELECT 'user', u.id, u.username::text, s.role FROM knowledge_base_shares s JOIN users u ON u.id = s.user_id WHERE s.knowledge_base_id = $1
			 ORDER BY 1, 3",
		)
		.bind(self.id)
		.fetch_all(pool)
		.await?;
		let owner = self.owner_target(pool).await?;
		let everyone = self.shared_with_everyone.then_some(AssignmentTarget {
			kind: AssignmentKind::Everyone,
			id: None,
			label: String::new(),
			inherited: false,
			owner: false,
			role: None,
		});
		let shares = rows.into_iter().map(|(kind, id, label, role)| AssignmentTarget {
			kind: AssignmentKind::from_stored(&kind),
			id: Some(id),
			label,
			inherited: false,
			owner: false,
			role: Some(AssignmentRole::from_stored(&role)),
		});
		Ok(AssignmentsResponse {
			kinds: actor.share_kinds(self),
			single: false,
			roles: vec![AssignmentRole::Viewer, AssignmentRole::Editor],
			targets: owner.into_iter().chain(everyone).chain(shares).collect(),
		})
	}

	async fn owner_target(&self, pool: &PgPool) -> Result<Option<AssignmentTarget>, sqlx::Error> {
		match (self.scope, self.owner_id, self.team_id) {
			(KnowledgeScope::Personal, Some(user_id), _) => {
				let name: Option<String> = sqlx::query_scalar("SELECT username::text FROM users WHERE id = $1").bind(user_id).fetch_optional(pool).await?;
				Ok(name.map(|label| AssignmentTarget {
					kind: AssignmentKind::User,
					id: Some(user_id),
					label,
					inherited: false,
					owner: true,
					role: None,
				}))
			}
			(KnowledgeScope::Team, _, Some(team_id)) => Ok(Some(AssignmentTarget {
				kind: AssignmentKind::Team,
				id: Some(team_id),
				label: self.team_name.clone().unwrap_or_default(),
				inherited: false,
				owner: true,
				role: None,
			})),
			_ => Ok(None),
		}
	}

	/// Makes the request's targets the only ones the knowledge base is shared with, audited in the same transaction.
	///
	/// # Errors
	///
	/// Returns `Forbidden`, `InvalidAssignment` or a database error.
	pub async fn replace_assignments(&self, pool: &PgPool, actor: &KnowledgeActor, request: &ReplaceAssignmentsRequest) -> Result<(), RagError> {
		let allowed_kinds = actor.share_kinds(self);
		if allowed_kinds.is_empty() {
			return Err(RagError::Forbidden);
		}
		let filtered: Vec<_> = request
			.targets
			.iter()
			.filter(|target| !self.is_owner_ref(target.kind, target.id))
			.cloned()
			.collect();
		let check_req = ReplaceAssignmentsRequest { targets: filtered };
		let everyone = check_req.targets.iter().filter(|target| target.kind == AssignmentKind::Everyone);
		if !check_req.is_valid_for(&allowed_kinds) || everyone.clone().any(|target| target.role == Some(AssignmentRole::Editor)) {
			return Err(RagError::InvalidAssignment);
		}
		if !actor.manage
			&& check_req
				.targets
				.iter()
				.any(|target| target.kind == AssignmentKind::Team && target.id.is_none_or(|id| !actor.admin_team_ids.contains(&id)))
		{
			return Err(RagError::Forbidden);
		}
		let shared_with_everyone = everyone.count() > 0;
		let shares: BTreeMap<(&str, Uuid), &str> = check_req
			.targets
			.iter()
			.filter_map(|target| {
				let kind = match target.kind {
					AssignmentKind::Team => "team",
					AssignmentKind::Role => "role",
					AssignmentKind::User => "user",
					AssignmentKind::Key | AssignmentKind::Everyone => return None,
				};
				Some(((kind, target.id?), target.role.unwrap_or(AssignmentRole::Viewer).as_str()))
			})
			.collect();
		let kinds: Vec<&str> = shares.keys().map(|(kind, _)| *kind).collect();
		let ids: Vec<Uuid> = shares.keys().map(|(_, id)| *id).collect();
		let roles: Vec<&str> = shares.values().copied().collect();
		let mut tx = pool.begin().await?;
		sqlx::query("UPDATE knowledge_bases SET shared_with_everyone = $2, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(shared_with_everyone)
			.execute(&mut *tx)
			.await?;
		sqlx::query("DELETE FROM knowledge_base_shares WHERE knowledge_base_id = $1")
			.bind(self.id)
			.execute(&mut *tx)
			.await?;
		sqlx::query(
			"INSERT INTO knowledge_base_shares (knowledge_base_id, user_id, team_id, role_id, role)
			 SELECT $1, CASE WHEN kind = 'user' THEN id END, CASE WHEN kind = 'team' THEN id END, CASE WHEN kind = 'role' THEN id END, role
			 FROM unnest($2::text[], $3::uuid[], $4::text[]) AS target(kind, id, role)",
		)
		.bind(self.id)
		.bind(&kinds)
		.bind(&ids)
		.bind(&roles)
		.execute(&mut *tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::InvalidAssignment))?;
		let targets: Vec<serde_json::Value> = kinds
			.iter()
			.zip(&ids)
			.zip(&roles)
			.map(|((kind, id), role)| serde_json::json!({ "kind": kind, "id": id, "role": role }))
			.collect();
		let metadata = serde_json::json!({ "name": self.name, "everyone": shared_with_everyone, "targets": targets });
		audit(&mut tx, "knowledge_base_sharing_updated", &actor.user_id, &self.id, metadata).await?;
		tx.commit().await?;
		Ok(())
	}

	fn is_owner_ref(&self, kind: AssignmentKind, id: Option<Uuid>) -> bool {
		(self.scope == KnowledgeScope::Personal && kind == AssignmentKind::User && id.is_some() && id == self.owner_id)
			|| (self.scope == KnowledgeScope::Team && kind == AssignmentKind::Team && id.is_some() && id == self.team_id)
	}

	/// Searches the knowledge base for `query` with the access `labels` of whoever the search is for, billing the query embedding to `user_id` as `rag_query`.
	///
	/// A store that cannot be reached (or is re-indexing) is reported in the response, not as an error.
	///
	/// # Errors
	///
	/// Returns why the query could not be embedded, or a database error.
	pub async fn search(&self, state: &JobState, user_id: &Uuid, query: &str, labels: &[String]) -> Result<KnowledgeSearchResponse, RagError> {
		let connection = RagConnection::find(&state.db, &self.connection_id).await?.ok_or(RagError::NotFound)?;
		if self.is_managed() {
			if connection.is_reindexing(&state.db).await? {
				return Ok(KnowledgeSearchResponse {
					error: Some(StoreError::Reindexing),
					hits: Vec::new(),
					access: None,
				});
			}
			let settings = connection.embedding_settings().ok_or(RagError::ConnectionNotReady)?;
			let vector = embed(state, user_id, &settings, query, "rag_query").await?;
			let params = ManagedHybridQuery {
				base: self,
				connection: &connection,
				query,
				vector: &vector,
				limit: TEST_SEARCH_HITS,
				labels,
			};
			return Ok(match hybrid_search_managed(state, &params).await {
				Ok(hits) => KnowledgeSearchResponse {
					error: None,
					hits: hits.into_iter().map(|item| item.hit).collect(),
					access: None,
				},
				Err(error) => KnowledgeSearchResponse {
					error: Some(error),
					hits: Vec::new(),
					access: None,
				},
			});
		}
		let view = self.external_view().ok_or(RagError::NotFound)?;
		let vector = embed(state, user_id, &view.embedding, query, "rag_query").await?;
		let result = match VectorStore::connect(&connection, &state.db, None).await {
			Ok(store) => store.search(view, &vector, TEST_SEARCH_HITS, labels).await,
			Err(error) => Err(error),
		};
		Ok(match result {
			Ok(records) => KnowledgeSearchResponse {
				error: None,
				hits: records.iter().filter_map(|(payload, score)| view.hit(payload, *score)).collect(),
				access: None,
			},
			Err(error) => KnowledgeSearchResponse {
				error: Some(error),
				hits: Vec::new(),
				access: None,
			},
		})
	}

	/// The embedding settings for this knowledge base: the external view's or the managed connection's.
	#[must_use]
	pub fn embedding_for(&self, connection: Option<&RagConnection>) -> Option<EmbeddingSettings> {
		if let Some(view) = self.external_view() {
			Some(view.embedding)
		} else {
			connection.and_then(RagConnection::embedding_settings)
		}
	}

	/// The viewer's view: knowledge admins also see the connection and external source.
	#[must_use]
	pub fn to_response(&self, actor: &KnowledgeActor) -> KnowledgeBaseResponse {
		KnowledgeBaseResponse {
			id: self.id,
			name: self.name.clone(),
			description: self.description.clone(),
			mode: self.connection_mode,
			scope: self.scope,
			team_id: self.team_id,
			team_name: self.team_name.clone(),
			owned: actor.owns(self),
			search_language: self.search_language.clone(),
			document_count: u32::try_from(self.document_count).unwrap_or(u32::MAX),
			can_edit: actor.can_edit(self),
			can_manage: actor.can_manage(self),
			usable: self.usable,
			connection: actor.view_all.then(|| KnowledgeBaseConnection {
				id: self.connection_id,
				name: self.connection_name.clone(),
				kind: self.connection_kind,
			}),
			view: actor.view_all.then(|| self.external_view().cloned()).flatten(),
			allowed_model_ids: None,
		}
	}
}
