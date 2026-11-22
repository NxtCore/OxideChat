//! Knowledge bases on read-only connections: who may see them, sharing and test search.

use super::{
	ConnectionKind, ConnectionMode, ExternalView, KnowledgeBaseConnection, KnowledgeBaseResponse, KnowledgeSearchResponse, RagConnection, RagError,
	SaveKnowledgeBaseRequest, VectorStore, embed, probe_embedding,
};
use crate::logging::AuditLog;
use crate::types::{AssignmentKind, AssignmentRole, AssignmentTarget, AssignmentsResponse, AuditEntry, JobState, ReplaceAssignmentsRequest};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

const AUDIT_TARGET: &str = "knowledge_base";

/// Hits a test search returns.
pub const TEST_SEARCH_HITS: u32 = 8;

macro_rules! columns {
	() => {
		"kb.id, kb.name, kb.description, kb.connection_id, kb.view, kb.shared_with_everyone, c.name AS connection_name, c.kind AS connection_kind,
		 EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND s.role = 'editor' AND (
			s.user_id = $1
			OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
		 )) AS editor,
		 (kb.shared_with_everyone OR EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND (
			s.user_id = $1
			OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
		 ))) AS usable
		 FROM knowledge_bases kb JOIN rag_connections c ON c.id = kb.connection_id"
	};
}

macro_rules! visible {
	() => {
		"(kb.shared_with_everyone OR EXISTS (SELECT 1 FROM knowledge_base_shares s WHERE s.knowledge_base_id = kb.id AND (
			s.user_id = $1
			OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1)
			OR s.role_id IN (SELECT role_id FROM user_roles WHERE user_id = $1)
		)))"
	};
}

/// A knowledge base as loaded for one viewer; `editor` says a share gives them the editor role.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeBase {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub connection_id: Uuid,
	pub view: Json<ExternalView>,
	pub shared_with_everyone: bool,
	pub connection_name: String,
	pub connection_kind: ConnectionKind,
	pub editor: bool,
	/// Shared with the viewer, so they may search it; knowledge admins see others too.
	pub usable: bool,
}

/// Who is acting on knowledge bases: `manage` is `admin.rag.edit`, `view_all` is `admin.rag.view`.
#[derive(Debug, Clone, Copy)]
pub struct KnowledgeActor {
	pub user_id: Uuid,
	pub manage: bool,
	pub view_all: bool,
}

impl KnowledgeActor {
	/// Whether the actor may change the name and description of `knowledge_base`.
	#[must_use]
	pub const fn can_edit(&self, knowledge_base: &KnowledgeBase) -> bool {
		self.manage || knowledge_base.editor
	}
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

/// Checks `view` against its store: a read-only connection, an existing vector and an embedding model whose vectors have the collection's size.
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

impl KnowledgeBase {
	/// What a knowledge base can be shared with.
	pub const ASSIGNMENT_KINDS: [AssignmentKind; 4] = [AssignmentKind::Everyone, AssignmentKind::Team, AssignmentKind::Role, AssignmentKind::User];

	/// Knowledge bases the actor may see, by name: every one for knowledge admins, otherwise those shared with them.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool, actor: &KnowledgeActor) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", columns!(), " WHERE $2 OR ", visible!(), " ORDER BY lower(kb.name), kb.id"))
			.bind(actor.user_id)
			.bind(actor.view_all)
			.fetch_all(pool)
			.await
	}

	/// One knowledge base the actor may see.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, actor: &KnowledgeActor, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", columns!(), " WHERE kb.id = $3 AND ($2 OR ", visible!(), ")"))
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
	pub async fn accessible(pool: &PgPool, user_id: &Uuid, ids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
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
		.fetch_all(pool)
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
	pub async fn preset_ids(pool: &PgPool, preset_id: &Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
		sqlx::query_scalar("SELECT knowledge_base_id FROM preset_knowledge_bases WHERE preset_id = $1 ORDER BY knowledge_base_id")
			.bind(preset_id)
			.fetch_all(pool)
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

	/// Stores a new knowledge base shared with nobody; the view must have been checked with [`check_view`].
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
		let id: Uuid = sqlx::query_scalar("INSERT INTO knowledge_bases (name, description, connection_id, view, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
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

	/// Changes name and description and, with `view`, the source; the view must have been checked with [`check_view`].
	///
	/// # Errors
	///
	/// Returns `Forbidden` when an editor sends a view, `NotFound` when the knowledge base is gone, or a database error.
	pub async fn update(&self, pool: &PgPool, actor: &KnowledgeActor, request: &SaveKnowledgeBaseRequest, view: Option<&ExternalView>) -> Result<(), RagError> {
		if !actor.can_edit(self) || (view.is_some() && !actor.manage) {
			return Err(RagError::Forbidden);
		}
		request.validate()?;
		let view = view.unwrap_or(&self.view.0);
		let mut tx = pool.begin().await?;
		let updated = sqlx::query("UPDATE knowledge_bases SET name = $2, description = $3, connection_id = $4, view = $5, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(request.name.trim())
			.bind(request.description())
			.bind(view.connection_id)
			.bind(Json(view))
			.execute(&mut *tx)
			.await?;
		if updated.rows_affected() == 0 {
			return Err(RagError::NotFound);
		}
		audit(&mut tx, "knowledge_base_updated", &actor.user_id, &self.id, view_metadata(request.name.trim(), view)).await?;
		tx.commit().await?;
		Ok(())
	}

	/// Deletes the knowledge base and its shares.
	///
	/// # Errors
	///
	/// Returns `Forbidden` unless the actor manages knowledge, or a database error.
	pub async fn delete(&self, pool: &PgPool, actor: &KnowledgeActor) -> Result<(), RagError> {
		if !actor.manage {
			return Err(RagError::Forbidden);
		}
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM knowledge_bases WHERE id = $1").bind(self.id).execute(&mut *tx).await?;
		audit(&mut tx, "knowledge_base_deleted", &actor.user_id, &self.id, serde_json::json!({ "name": self.name })).await?;
		tx.commit().await?;
		Ok(())
	}

	/// Who may use the knowledge base, in the shared assignment shape; only knowledge admins may change it.
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
			kinds: if actor.manage { Self::ASSIGNMENT_KINDS.to_vec() } else { Vec::new() },
			single: false,
			roles: vec![AssignmentRole::Viewer, AssignmentRole::Editor],
			targets: everyone.into_iter().chain(shares).collect(),
		})
	}

	/// Makes the request's targets the only ones the knowledge base is shared with, audited in the same transaction.
	///
	/// # Errors
	///
	/// Returns `Forbidden` unless the actor manages knowledge, `InvalidAssignment` for another kind of target, an editor
	/// role for everyone or an unknown target, or a database error.
	pub async fn replace_assignments(&self, pool: &PgPool, actor: &KnowledgeActor, request: &ReplaceAssignmentsRequest) -> Result<(), RagError> {
		if !actor.manage {
			return Err(RagError::Forbidden);
		}
		let everyone = request.targets.iter().filter(|target| target.kind == AssignmentKind::Everyone);
		if !request.is_valid_for(&Self::ASSIGNMENT_KINDS) || everyone.clone().any(|target| target.role == Some(AssignmentRole::Editor)) {
			return Err(RagError::InvalidAssignment);
		}
		let shared_with_everyone = everyone.count() > 0;
		let shares: BTreeMap<(&str, Uuid), &str> = request
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

	/// Searches the knowledge base for `query`, billing the query embedding to `user_id` as `rag_query`.
	///
	/// A store that cannot be reached is reported in the response, not as an error.
	///
	/// # Errors
	///
	/// Returns why the query could not be embedded, or a database error.
	pub async fn search(&self, state: &JobState, user_id: &Uuid, query: &str, limit: u32) -> Result<KnowledgeSearchResponse, RagError> {
		let connection = RagConnection::find(&state.db, &self.connection_id).await?.ok_or(RagError::NotFound)?;
		let vector = embed(state, user_id, &self.view.embedding, query, "rag_query").await?;
		let result = match VectorStore::connect(&connection, &state.db, None).await {
			Ok(store) => store.search(&self.view, &vector, limit).await,
			Err(error) => Err(error),
		};
		Ok(match result {
			Ok(records) => KnowledgeSearchResponse {
				error: None,
				hits: records.iter().filter_map(|(payload, score)| self.view.hit(payload, *score)).collect(),
			},
			Err(error) => KnowledgeSearchResponse {
				error: Some(error),
				hits: Vec::new(),
			},
		})
	}

	/// The viewer's view: knowledge admins also see the connection and source.
	#[must_use]
	pub fn to_response(&self, actor: &KnowledgeActor) -> KnowledgeBaseResponse {
		KnowledgeBaseResponse {
			id: self.id,
			name: self.name.clone(),
			description: self.description.clone(),
			can_edit: actor.can_edit(self),
			can_manage: actor.manage,
			usable: self.usable,
			connection: actor.view_all.then(|| KnowledgeBaseConnection {
				id: self.connection_id,
				name: self.connection_name.clone(),
				kind: self.connection_kind,
			}),
			view: actor.view_all.then(|| self.view.0.clone()),
		}
	}
}
