//! Admin routes for the knowledge setup: connections, embedding and rerank models.

use crate::types::consts::{ADMIN_RAG_EDIT, ADMIN_RAG_VIEW};
use crate::types::rag::{ConnectionMode, ConnectionTestResponse, RagConfig, RagConnection, SaveConnectionRequest, StoreError, UpdateRagSetupRequest, VectorStore, check_reranker, probe_embedding};
use crate::types::{AppError, AuthUser, JobState, json_ok};
use crate::utils::response::ErrorCode;
use axum::{
	Json,
	extract::{Path, State},
	response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

/// GET /api/v1/admin/rag
pub async fn get_setup(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_VIEW).await?;
	json_ok(RagConfig::setup(&state.db).await?)
}

/// PUT /api/v1/admin/rag
pub async fn update_setup(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<UpdateRagSetupRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_EDIT).await?;
	if let Some(model_id) = &req.reranker.model_id {
		check_reranker(&state.db, model_id).await?;
	}
	RagConfig::save(&state.db, &auth.id, &req).await?;
	json_ok(RagConfig::setup(&state.db).await?)
}

/// POST /api/v1/admin/rag/connections
///
/// A managed connection with an embedding model sends one test embedding, billed to the admin.
pub async fn create_connection(State(state): State<Arc<JobState>>, auth: AuthUser, Json(req): Json<SaveConnectionRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_EDIT).await?;
	req.validate(req.kind, req.mode, false)?;
	let vector_size = match &req.embedding {
		Some(settings) => Some(probe_embedding(&state, &auth.id, settings).await?),
		None => None,
	};
	RagConnection::create(&state.db, &auth.id, &req, vector_size).await?;
	json_ok(RagConfig::setup(&state.db).await?)
}

/// PUT /api/v1/admin/rag/connections/{id}
///
/// `kind` and `mode` are ignored. A missing secret keeps the stored one unless the host changes.
pub async fn update_connection(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<SaveConnectionRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_EDIT).await?;
	let existing = RagConnection::find(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	req.validate(existing.kind, existing.mode, existing.secret.is_some() && req.secret.is_none())?;
	if existing.redirects_secret(&req) {
		return Err(ErrorCode::SecretRequiredForNewHost.into());
	}
	let vector_size = match &req.embedding {
		Some(settings) if req.embedding != existing.embedding_settings() || existing.vector_size.is_none() => Some(probe_embedding(&state, &auth.id, settings).await?),
		_ => None,
	};
	existing.update(&state.db, &auth.id, &req, vector_size).await?;
	json_ok(RagConfig::setup(&state.db).await?)
}

/// DELETE /api/v1/admin/rag/connections/{id}
pub async fn delete_connection(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_EDIT).await?;
	if !RagConnection::delete(&state.db, &auth.id, &id).await? {
		return Err(ErrorCode::NotFound.into());
	}
	json_ok(RagConfig::setup(&state.db).await?)
}

/// POST /api/v1/admin/rag/connections/{id}/test
///
/// Reports the store's version and, for read-only connections, its collections. Never returns the server's own error text.
pub async fn test_connection(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_RAG_VIEW).await?;
	let connection = RagConnection::find(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	let env_url = RagConfig::get(&state.db).await?.env_url_for(&connection);
	let result = test(&connection, &state, env_url.as_deref()).await;
	json_ok(match result {
		Ok((version, collections)) => ConnectionTestResponse { ok: true, error: None, version: Some(version), collections },
		Err(error) => ConnectionTestResponse { ok: false, error: Some(error), version: None, collections: Vec::new() },
	})
}

async fn test(connection: &RagConnection, state: &JobState, env_url: Option<&str>) -> Result<(String, Vec<String>), StoreError> {
	let store = VectorStore::connect(connection, &state.db, env_url).await?;
	let version = store.health().await?;
	let collections = match connection.mode {
		ConnectionMode::ReadOnly => store.list_collections().await?,
		ConnectionMode::Managed => Vec::new(),
	};
	Ok((version, collections))
}
