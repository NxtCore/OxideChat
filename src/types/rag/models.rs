//! Checks the embedding and rerank models the setup page picks.

use super::chunker::estimate_tokens;
use super::{EmbeddingSettings, RagError};
use crate::ai::provider_to_config;
use crate::types::gateway::{MediaKind, knowledge_estimate, model_type};
use crate::types::models::{Model, ModelPricing};
use crate::types::providers::Provider;
use crate::types::{BUDGET_RESERVATION_METADATA_KEY, BudgetAdmission, BudgetError, BudgetSubject, JobState, TokenEstimate};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{StreamExt, stream};
use omniference::types::{EmbeddingInput, EmbeddingRequestIR, EmbeddingVector, ModelRef, RerankRequestIR};
use sqlx::PgPool;
use sqlx::types::Json;
use std::collections::BTreeMap;
use uuid::Uuid;

const TEST_INPUT: &str = "OxideChat knowledge test";
const EMBED_BATCH_MAX_INPUTS: usize = 64;
const EMBED_BATCH_MAX_TOKENS: usize = 16_000;
const EMBED_IN_FLIGHT_PER_DOC: usize = 2;

async fn load(pool: &PgPool, model_id: &Uuid, expected_type: &str, wrong_type: RagError) -> Result<(Model, Provider), RagError> {
	let model = Model::find_by_id(pool, model_id).await?.ok_or(RagError::ModelUnavailable)?;
	let provider = Provider::find_for_admin(pool, &model.provider_id).await?.ok_or(RagError::ModelUnavailable)?;
	if !model.is_enabled || !provider.is_enabled {
		return Err(RagError::ModelUnavailable);
	}
	let kind = sqlx::query!(
		r#"SELECT COALESCE(mc.capabilities, m.capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
		   COALESCE(mc.output_modalities, m.output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>"
		   FROM models m LEFT JOIN model_configs mc ON mc.model_id = m.id AND mc.owner_id IS NULL
		   WHERE m.id = $1"#,
		model.id
	)
	.fetch_optional(pool)
	.await?
	.ok_or(RagError::ModelUnavailable)?;
	if model_type(&kind.capabilities.0, &kind.output_modalities.0) != expected_type {
		return Err(wrong_type);
	}
	Ok((model, provider))
}

/// Checks that `model_id` is an enabled reranking model.
///
/// # Errors
///
/// Returns `ModelUnavailable`, `NotRerankingModel` or a database error.
pub async fn check_reranker(pool: &PgPool, model_id: &Uuid) -> Result<(), RagError> {
	load(pool, model_id, "reranking", RagError::NotRerankingModel).await.map(|_| ())
}

/// Sends one embedding with `settings`, billed to `user_id`, and returns the vector size it came back with.
///
/// # Errors
///
/// Returns why the model cannot be used, the budget refused it, the provider failed or the size is not the requested one.
pub async fn probe_embedding(state: &JobState, user_id: &Uuid, settings: &EmbeddingSettings) -> Result<u32, RagError> {
	let vector = embed(state, user_id, settings, TEST_INPUT, "rag_setup").await?;
	match (u32::try_from(vector.len()), settings.dimensions) {
		(Ok(size), Some(requested)) if size != requested => Err(RagError::DimensionsMismatch),
		(Ok(size), _) if (1..=super::MAX_DIMENSIONS).contains(&size) => Ok(size),
		_ => Err(RagError::InvalidDimensions),
	}
}

/// Embeds `text` with `settings`, reserved against and billed to `user_id` as `request_type`.
///
/// # Errors
///
/// Returns why the model cannot be used, the budget refused it or the provider failed.
pub async fn embed(state: &JobState, user_id: &Uuid, settings: &EmbeddingSettings, text: &str, request_type: &str) -> Result<Vec<f32>, RagError> {
	let (model, provider) = load(&state.db, &settings.model_id, "embedding", RagError::NotEmbeddingModel).await?;
	let (admission, metadata) = reserve(state, user_id, &model, knowledge_estimate(text.len() as u64, MediaKind::Embeddings), request_type).await?;
	let request = EmbeddingRequestIR {
		model: model_ref(&model, &provider).await?,
		input: EmbeddingInput::Text(text.to_string()),
		dimensions: settings.dimensions,
		encoding_format: None,
		raw_request: None,
		metadata,
	};
	let response = crate::ai::get()
		.embeddings(request)
		.await
		.map_err(|error| RagError::EmbeddingFailed(error.to_string()))?;
	drop(admission);
	response
		.data
		.into_iter()
		.next()
		.map(|item| floats(item.embedding))
		.filter(|vector| !vector.is_empty())
		.ok_or_else(|| RagError::EmbeddingFailed("no embedding returned".to_string()))
}

/// Normalizes `vector` to unit L2 length in place; returns `false` if empty, non-finite or zero-length.
#[must_use]
pub fn normalize_vector(vector: &mut [f32]) -> bool {
	if vector.is_empty() || !vector.iter().all(|value| value.is_finite()) {
		return false;
	}
	let norm_sq: f64 = vector.iter().map(|value| f64::from(*value) * f64::from(*value)).sum();
	let norm = norm_sq.sqrt();
	if !norm.is_finite() || norm <= f64::EPSILON {
		return false;
	}
	for value in vector.iter_mut() {
		*value = (f64::from(*value) / norm) as f32;
	}
	true
}

/// Groups `inputs` into batches of at most 64 inputs and about 16k tokens.
#[must_use]
pub fn split_embedding_batches(inputs: &[String]) -> Vec<Vec<String>> {
	let mut batches = Vec::new();
	let mut current = Vec::new();
	let mut tokens = 0usize;
	for input in inputs {
		let input_tokens = estimate_tokens(input);
		if !current.is_empty() && (current.len() >= EMBED_BATCH_MAX_INPUTS || tokens + input_tokens > EMBED_BATCH_MAX_TOKENS) {
			batches.push(std::mem::take(&mut current));
			tokens = 0;
		}
		tokens += input_tokens;
		current.push(input.clone());
	}
	if !current.is_empty() {
		batches.push(current);
	}
	batches
}

struct BatchEmbedTarget<'a> {
	model: &'a Model,
	target_ref: &'a ModelRef,
	dimensions: Option<u32>,
	request_type: &'a str,
}

/// Embeds `inputs` in batches of at most 64 inputs / ~16k tokens, up to 2 in flight, each batch reserved against `user_id` as `request_type`.
///
/// # Errors
///
/// Returns why the model cannot be used, the budget refused a batch or the provider failed.
pub async fn embed_batch(state: &JobState, user_id: &Uuid, settings: &EmbeddingSettings, inputs: &[String], request_type: &str) -> Result<Vec<Vec<f32>>, RagError> {
	if inputs.is_empty() {
		return Ok(Vec::new());
	}
	let (model, provider) = load(&state.db, &settings.model_id, "embedding", RagError::NotEmbeddingModel).await?;
	let target_ref = model_ref(&model, &provider).await?;
	let target = BatchEmbedTarget {
		model: &model,
		target_ref: &target_ref,
		dimensions: settings.dimensions,
		request_type,
	};
	let batches = split_embedding_batches(inputs);
	let mut results = stream::iter(batches.into_iter().map(|batch| embed_one_batch(state, user_id, &target, batch))).buffered(EMBED_IN_FLIGHT_PER_DOC);
	let mut vectors = Vec::with_capacity(inputs.len());
	while let Some(batch) = results.next().await {
		vectors.extend(batch?);
	}
	Ok(vectors)
}

async fn embed_one_batch(state: &JobState, user_id: &Uuid, target: &BatchEmbedTarget<'_>, batch: Vec<String>) -> Result<Vec<Vec<f32>>, RagError> {
	let expected = batch.len();
	let bytes: u64 = batch.iter().map(|text| text.len() as u64).sum();
	let (admission, metadata) = reserve(state, user_id, target.model, knowledge_estimate(bytes, MediaKind::Embeddings), target.request_type).await?;
	let request = EmbeddingRequestIR {
		model: target.target_ref.clone(),
		input: EmbeddingInput::Texts(batch),
		dimensions: target.dimensions,
		encoding_format: None,
		raw_request: None,
		metadata,
	};
	let response = crate::ai::get()
		.embeddings(request)
		.await
		.map_err(|error| RagError::EmbeddingFailed(error.to_string()))?;
	drop(admission);
	let mut items = response.data;
	items.sort_by_key(|item| item.index);
	let vectors: Vec<Vec<f32>> = items.into_iter().map(|item| floats(item.embedding)).filter(|vector| !vector.is_empty()).collect();
	if vectors.len() != expected {
		return Err(RagError::EmbeddingFailed("embedding batch count mismatch".to_string()));
	}
	Ok(vectors)
}

/// Estimates USD cost of embedding `total_tokens` with `model_id`.
///
/// # Errors
///
/// Returns a database error when pricing cannot be read.
pub async fn estimate_embedding_cost(pool: &PgPool, model_id: &Uuid, total_tokens: u64) -> Result<Option<f64>, sqlx::Error> {
	let estimate = knowledge_estimate(total_tokens.saturating_mul(4), MediaKind::Embeddings);
	let cost = ModelPricing::estimate_cost(pool, model_id, estimate).await?;
	Ok(cost.to_string().parse::<f64>().ok())
}

/// Sorts `documents` by relevance to `query` with the reranker `model_id`, billed to `user_id` as `rag_query`; returns document indexes, most relevant first.
///
/// # Errors
///
/// Returns why the model cannot be used, the budget refused it or the provider failed.
pub async fn rerank(state: &JobState, user_id: &Uuid, model_id: &Uuid, query: &str, documents: &[&str]) -> Result<Vec<usize>, RagError> {
	let (model, provider) = load(&state.db, model_id, "reranking", RagError::NotRerankingModel).await?;
	let bytes = query.len() + documents.iter().map(|document| document.len()).sum::<usize>();
	let (admission, metadata) = reserve(state, user_id, &model, knowledge_estimate(bytes as u64, MediaKind::Rerank), "rag_query").await?;
	let request = RerankRequestIR {
		model: model_ref(&model, &provider).await?,
		query: query.to_string(),
		documents: documents.iter().map(|document| serde_json::Value::String((*document).to_string())).collect(),
		top_n: None,
		return_documents: Some(false),
		raw_request: None,
		metadata,
	};
	let response = crate::ai::get().rerank(request).await.map_err(|error| RagError::EmbeddingFailed(error.to_string()))?;
	drop(admission);
	let mut results = response.results;
	results.sort_by(|a, b| b.relevance_score.total_cmp(&a.relevance_score));
	Ok(results.into_iter().filter_map(|result| usize::try_from(result.index).ok()).collect())
}

async fn reserve(state: &JobState, user_id: &Uuid, model: &Model, estimate: TokenEstimate, request_type: &str) -> Result<(BudgetAdmission, BTreeMap<String, String>), RagError> {
	let admission = match state
		.budget_ledger
		.reserve_for_model(&state.db, BudgetSubject::user(*user_id), &model.id, estimate)
		.await
	{
		Ok(admission) => admission,
		Err(BudgetError::Exceeded) => return Err(RagError::BudgetExceeded),
		Err(BudgetError::Database(error)) => return Err(RagError::Database(error)),
		Err(BudgetError::Unavailable) => return Err(RagError::Database(sqlx::Error::PoolClosed)),
	};
	let mut metadata = BTreeMap::from([
		("oxide_user_id".to_string(), user_id.to_string()),
		("oxide_model_id".to_string(), model.id.to_string()),
		("oxide_request_type".to_string(), request_type.to_string()),
	]);
	if let Some(reservation) = &admission.reservation {
		metadata.insert(BUDGET_RESERVATION_METADATA_KEY.to_string(), reservation.id.to_string());
	}
	Ok((admission, metadata))
}

async fn model_ref(model: &Model, provider: &Provider) -> Result<ModelRef, RagError> {
	let engine = crate::ai::get();
	let registered = engine.get_provider(&provider.name).await;
	let provider_config = match registered {
		Some(config) => config,
		None => provider_to_config(provider).map_err(|error| RagError::EmbeddingFailed(error.to_string()))?,
	};
	let prefix = format!("{}/", provider.name.to_lowercase());
	let bare = model.model_id.strip_prefix(&prefix).unwrap_or(&model.model_id);
	Ok(ModelRef {
		alias: model.display_name.clone(),
		provider: provider_config,
		model_id: bare.to_string(),
		input_modalities: Vec::new(),
		output_modalities: Vec::new(),
	})
}

/// The floats of a vector, also when the provider sent it as base64 little-endian `f32`s.
#[must_use]
pub fn floats(vector: EmbeddingVector) -> Vec<f32> {
	match vector {
		EmbeddingVector::Float(values) if values.iter().all(|value| value.is_finite()) => values,
		EmbeddingVector::Float(_) => Vec::new(),
		EmbeddingVector::Base64(encoded) => BASE64
			.decode(encoded)
			.map(|bytes| {
				let (chunks, _) = bytes.as_chunks::<4>();
				let values: Vec<f32> = chunks.iter().map(|chunk| f32::from_le_bytes(*chunk)).collect();
				if values.iter().all(|value| value.is_finite()) { values } else { Vec::new() }
			})
			.unwrap_or_default(),
	}
}
