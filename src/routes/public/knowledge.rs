//! Knowledge base routes: managed & external bases, documents, sharing, access and test search.

use crate::types::rag::{
	DocumentUploadQuery, KnowledgeActor, KnowledgeBase, KnowledgeBaseResponse, KnowledgeSearchRequest, KnowledgeSearchResponse, LabelQuery, ModelTargets, RagDocument, RagError,
	RoutedModel, SaveKnowledgeBaseRequest, check_embedding_allowed, check_label, check_view,
};
use crate::types::{AppError, AuthUser, JobState, ReplaceAssignmentsRequest, json_ok};
use axum::{
	Json,
	body::Bytes,
	extract::{Path, Query, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

async fn actor(state: &JobState, auth: &AuthUser) -> Result<KnowledgeActor, AppError> {
	let permissions = auth.permissions(&state.db).await?;
	Ok(KnowledgeActor::for_user(&state.db, auth.id, &permissions).await?)
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

/// GET /api/v1/knowledge-bases/scopes
///
/// Scopes where the current user may create knowledge bases and whether managed storage is ready.
pub async fn get_scopes(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	json_ok(KnowledgeBase::scopes(&state.db, &actor).await?)
}

/// GET /api/v1/knowledge-bases
///
/// Every knowledge base for knowledge admins, otherwise those owned by or shared with the user.
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
/// Creates a managed knowledge base when `view` is omitted, or an external view (knowledge admins only) when `view` is given.
pub async fn create_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<SaveKnowledgeBaseRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	req.validate()?;
	let id = match &req.view {
		Some(raw_view) => {
			if !actor.manage {
				return Err(RagError::Forbidden.into());
			}
			let view = check_view(&state, &auth.id, raw_view).await?;
			KnowledgeBase::create(&state.db, &actor, &req, &view).await?
		}
		None => KnowledgeBase::create_managed(&state.db, &actor, &req).await?,
	};
	respond(&state, &actor, &id).await
}

/// PUT /api/v1/knowledge-bases/{id}
///
/// Editors change name, description and search language; a new `view` or `model_access` needs a knowledge admin.
pub async fn update_knowledge_base(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Json(req): Json<SaveKnowledgeBaseRequest>,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !actor.can_edit(&base) || ((req.view.is_some() || req.model_access.is_some()) && !actor.manage) {
		return Err(RagError::Forbidden.into());
	}
	req.validate()?;
	let view = match (&req.view, base.external_view()) {
		(Some(view), Some(existing)) if *view != *existing => {
			let targets = match &req.model_access {
				Some(targets) => targets.normalized()?,
				None => ModelTargets::load(&state.db, &[id]).await?.remove(&id).unwrap_or_default(),
			};
			check_embedding_allowed(&state.db, &targets, &view.embedding.model_id).await?;
			Some(check_view(&state, &auth.id, view).await?)
		}
		(Some(_), None) => return Err(RagError::NotReadOnly.into()),
		_ => None,
	};
	base.update(&state.db, &actor, &req, view.as_ref()).await?;
	respond(&state, &actor, &id).await
}

/// DELETE /api/v1/knowledge-bases/{id}
pub async fn delete_knowledge_base(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let queued = find(&state, &actor, &id).await?.delete(&state.db, &actor).await?;
	if queued {
		state.rag_ingest_notify.notify_one();
	}
	json_ok(serde_json::json!({ "deleted": true }))
}

/// GET /api/v1/knowledge-bases/{id}/assignments
pub async fn get_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !actor.view_all && !actor.can_manage(&base) {
		return Err(RagError::Forbidden.into());
	}
	json_ok(base.assignments(&state.db, &actor).await?)
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
/// A test search with the user's own access, for bases shared with or managed by them; the query embedding is billed to them.
/// With `as_user`, a knowledge admin gets exactly that user's hits and why (audited as `rag_search_as`).
pub async fn search(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<KnowledgeSearchRequest>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let query = req.query()?;
	let base = find(&state, &actor, &id).await?;
	if !actor.can_search(&base) {
		return Err(RagError::Forbidden.into());
	}
	let Some(user_id) = req.as_user else {
		let labels = base.held_labels(&state.db, &auth.id).await?;
		return json_ok(base.search(&state, &auth.id, query, &labels).await?);
	};
	if !actor.manage {
		return Err(RagError::Forbidden.into());
	}
	base.audit_search_as(&state.db, &auth.id, &user_id, query).await?;
	let (access, labels) = base.access_for(&state, &user_id).await?;
	let mut result = if access.usable {
		base.search(&state, &auth.id, query, &labels).await?
	} else {
		KnowledgeSearchResponse::default()
	};
	result.access = Some(access);
	json_ok(result)
}

/// GET /api/v1/knowledge-bases/{id}/access-labels
///
/// For knowledge admins on external bases: the labels found in the collection and the mapped ones, with how many hold each.
pub async fn access_labels(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	if !actor.view_all {
		return Err(RagError::Forbidden.into());
	}
	json_ok(find(&state, &actor, &id).await?.access_labels(&state).await?)
}

/// GET /api/v1/knowledge-bases/{id}/access-labels/assignments?label=…
pub async fn get_label_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Query(query): Query<LabelQuery>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	if !actor.view_all {
		return Err(RagError::Forbidden.into());
	}
	let label = check_label(&query.label)?;
	json_ok(find(&state, &actor, &id).await?.label_assignments(&state.db, &actor, label).await?)
}

/// PUT /api/v1/knowledge-bases/{id}/access-labels/assignments?label=…
pub async fn replace_label_assignments(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Query(query): Query<LabelQuery>,
	Json(req): Json<ReplaceAssignmentsRequest>,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	base.replace_label_assignments(&state.db, &actor, &query.label, &req).await?;
	json_ok(base.label_assignments(&state.db, &actor, &query.label).await?)
}

/// GET /api/v1/knowledge-bases/{id}/model-access
pub async fn get_model_access(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !actor.view_all && !actor.can_edit(&base) {
		return Err(RagError::Forbidden.into());
	}
	json_ok(base.model_access(&state.db, &actor).await?)
}

/// PUT /api/v1/knowledge-bases/{id}/model-access
pub async fn replace_model_access(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<ModelTargets>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	base.replace_model_access(&state.db, &actor, &req).await?;
	json_ok(base.model_access(&state.db, &actor).await?)
}

/// GET /api/v1/knowledge-bases/{id}/documents
pub async fn list_documents(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	if !base.is_managed() {
		return Err(RagError::NotManaged.into());
	}
	let docs: Vec<_> = RagDocument::list(&state.db, &base.id).await?.iter().map(RagDocument::to_response).collect();
	json_ok(docs)
}

/// POST /api/v1/knowledge-bases/{id}/documents?filename=…
pub async fn upload_document(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path(id): Path<Uuid>,
	Query(query): Query<DocumentUploadQuery>,
	bytes: Bytes,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	let doc = RagDocument::upload(&state, &actor, &base, &query.filename, &bytes).await?;
	json_ok(doc.to_response())
}

/// DELETE /api/v1/knowledge-bases/{id}/documents/{doc_id}
pub async fn delete_document(State(state): State<Arc<JobState>>, auth: AuthUser, Path((id, doc_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	RagDocument::mark_deleting(&state, &actor, &base, &doc_id).await?;
	json_ok(serde_json::json!({ "deleting": true }))
}

/// POST /api/v1/knowledge-bases/{id}/documents/{doc_id}/retry
pub async fn retry_document(State(state): State<Arc<JobState>>, auth: AuthUser, Path((id, doc_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	let doc = RagDocument::retry(&state, &actor, &base, &doc_id).await?;
	json_ok(doc.to_response())
}

/// GET /api/v1/knowledge-bases/{id}/documents/{doc_id}/assignments
pub async fn get_document_assignments(State(state): State<Arc<JobState>>, auth: AuthUser, Path((id, doc_id)): Path<(Uuid, Uuid)>) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	json_ok(RagDocument::assignments(&state.db, &actor, &base, &doc_id).await?)
}

/// PUT /api/v1/knowledge-bases/{id}/documents/{doc_id}/assignments
pub async fn replace_document_assignments(
	State(state): State<Arc<JobState>>,
	auth: AuthUser,
	Path((id, doc_id)): Path<(Uuid, Uuid)>,
	Json(req): Json<ReplaceAssignmentsRequest>,
) -> Result<Response, AppError> {
	let actor = actor(&state, &auth).await?;
	let base = find(&state, &actor, &id).await?;
	RagDocument::replace_assignments(&state, &actor, &base, &doc_id, &req).await?;
	json_ok(RagDocument::assignments(&state.db, &actor, &base, &doc_id).await?)
}
