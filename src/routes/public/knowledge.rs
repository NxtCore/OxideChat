//! Knowledge base routes: the bases a user may search, their sharing and a test search.

use crate::types::consts::{ADMIN_RAG_EDIT, ADMIN_RAG_VIEW};
use crate::types::rag::{
	KnowledgeActor, KnowledgeBase, KnowledgeBaseResponse, KnowledgeSearchRequest, ModelTargets, RagError, RoutedModel, SaveKnowledgeBaseRequest, TEST_SEARCH_HITS,
	check_embedding_allowed, check_view,
};
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

/// The viewer's view of `bases` with the models each one allows.
async fn responses(state: &JobState, actor: &KnowledgeActor, bases: &[KnowledgeBase]) -> Result<Vec<KnowledgeBaseResponse>, AppError> {
	let ids: Vec<Uuid> = bases.iter().map(|base| base.id).collect();
	let targets = ModelTargets::load(&state.db, &ids).await?;
	let models = if targets.is_empty() { Vec::new() } else { RoutedModel::all_enabled(&state.db).await? };
	Ok(bases
		.iter()
		.map(|base| KnowledgeBaseResponse {
			allowed_model_ids: targets.get(&base.id).and_then(|targets| targets.allowed_models(&models)),
			..base.to_response(actor)
		})
		.collect())
}

async fn respond(state: &JobState, actor: &KnowledgeActor, id: &Uuid) -> Result<Response, AppError> {
	let base = find(state, actor, id).await?;
	json_ok(responses(state, actor, std::slice::from_ref(&base)).await?.pop())
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
	json_ok(responses(&state, &actor, &bases).await?)
}

/// GET /api/v1/knowledge-bases/{id}
pub async fn get_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	respond(&state, &actor, &id).await
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
	respond(&state, &actor, &id).await
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
		Some(view) if *view != base.view.0 => {
			let targets = ModelTargets::load(&state.db, &[id]).await?.remove(&id).unwrap_or_default();
			check_embedding_allowed(&state.db, &targets, &view.embedding.model_id).await?;
			Some(check_view(&state, &auth.id, view).await?)
		}
		_ => None,
	};
	base.update(&state.db, &actor, &req, view.as_ref()).await?;
	respond(&state, &actor, &id).await
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
/// A test search as the user, for bases shared with them or by knowledge admins (`admin.rag.edit`); the query embedding is billed to them.
pub async fn search(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<KnowledgeSearchRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let query = req.query()?;
	let base = find(&state, &actor, &id).await?;
	if !actor.can_search(&base) {
		return Err(RagError::Forbidden.into());
	}
	json_ok(base.search(&state, &auth.id, query, TEST_SEARCH_HITS).await?)
}

/// GET /api/v1/knowledge-bases/{id}/model-access
///
/// For editors and knowledge admins: which providers, upstream providers and models may see the content.
pub async fn get_model_access(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !actor.view_all && !actor.can_edit(&base) {
		return Err(RagError::Forbidden.into());
	}
	json_ok(base.model_access(&state.db, &actor).await?)
}

/// PUT /api/v1/knowledge-bases/{id}/model-access
///
/// Whoever may edit the knowledge base replaces its model access; it must allow the embedding model.
pub async fn replace_model_access(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<ModelTargets>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	base.replace_model_access(&state.db, &actor, &req).await?;
	json_ok(base.model_access(&state.db, &actor).await?)
}
