//! Knowledge base routes: the bases a user may search, their sharing and a test search.

use crate::types::consts::{ADMIN_RAG_EDIT, ADMIN_RAG_VIEW};
use crate::types::rag::{KnowledgeActor, KnowledgeBase, KnowledgeSearchRequest, RagError, SaveKnowledgeBaseRequest, TEST_SEARCH_HITS, check_view};
use crate::types::{AppError, AuthUser, JobState, ReplaceAssignmentsRequest, User, json_ok};
use axum::{
	Json,
	extract::{Path, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

async fn actor(state: &JobState, auth: &AuthUser) -> Result<KnowledgeActor, AppError> {
	let permissions = auth.permissions(&state.db).await?;
	let holds = |permission: &str| permissions.iter().any(|held| User::permission_matches(held, permission));
	Ok(KnowledgeActor {
		user_id: auth.id,
		manage: holds(ADMIN_RAG_EDIT),
		view_all: holds(ADMIN_RAG_VIEW),
	})
}

async fn find(state: &JobState, actor: &KnowledgeActor, id: &Uuid) -> Result<KnowledgeBase, AppError> {
	Ok(KnowledgeBase::find(&state.db, actor, id).await?.ok_or(RagError::NotFound)?)
}

/// GET /api/v1/knowledge-bases
///
/// Every knowledge base for knowledge admins, otherwise those shared with the user.
pub async fn list_knowledge_bases(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let bases = KnowledgeBase::list(&state.db, &actor).await?;
	json_ok(bases.iter().map(|base| base.to_response(&actor)).collect::<Vec<_>>())
}

/// GET /api/v1/knowledge-bases/{id}
pub async fn get_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	json_ok(find(&state, &actor, &id).await?.to_response(&actor))
}

/// POST /api/v1/knowledge-bases
///
/// Knowledge admins only. Checks the collection and sends one test embedding billed to the admin; the new base is shared with nobody.
pub async fn create_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<SaveKnowledgeBaseRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	if !actor.manage {
		return Err(RagError::Forbidden.into());
	}
	req.validate()?;
	let view = check_view(&state, &auth.id, req.view.as_ref().ok_or(RagError::InvalidCollection)?).await?;
	let id = KnowledgeBase::create(&state.db, &actor, &req, &view).await?;
	json_ok(find(&state, &actor, &id).await?.to_response(&actor))
}

/// PUT /api/v1/knowledge-bases/{id}
///
/// Editors change name and description; a new `view` needs a knowledge admin and is checked like on create.
pub async fn update_knowledge_base(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Json(req): Json<SaveKnowledgeBaseRequest>,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !actor.can_edit(&base) || (req.view.is_some() && !actor.manage) {
		return Err(RagError::Forbidden.into());
	}
	req.validate()?;
	let view = match &req.view {
		Some(view) if *view != base.view.0 => Some(check_view(&state, &auth.id, view).await?),
		_ => None,
	};
	base.update(&state.db, &actor, &req, view.as_ref()).await?;
	json_ok(find(&state, &actor, &id).await?.to_response(&actor))
}

/// DELETE /api/v1/knowledge-bases/{id}
pub async fn delete_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	find(&state, &actor, &id).await?.delete(&state.db, &actor).await?;
	json_ok(serde_json::json!({ "deleted": true }))
}

/// GET /api/v1/knowledge-bases/{id}/assignments
pub async fn get_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	if !actor.view_all {
		return Err(RagError::Forbidden.into());
	}
	json_ok(find(&state, &actor, &id).await?.assignments(&state.db, &actor).await?)
}

/// PUT /api/v1/knowledge-bases/{id}/assignments
pub async fn replace_assignments(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Json(req): Json<ReplaceAssignmentsRequest>,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	base.replace_assignments(&state.db, &actor, &req).await?;
	json_ok(find(&state, &actor, &id).await?.assignments(&state.db, &actor).await?)
}

/// POST /api/v1/knowledge-bases/{id}/search
///
/// A test search as the user; the query embedding is billed to them.
pub async fn search(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<KnowledgeSearchRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let query = req.query()?;
	let base = find(&state, &actor, &id).await?;
	json_ok(base.search(&state, &auth.id, query, TEST_SEARCH_HITS).await?)
}
