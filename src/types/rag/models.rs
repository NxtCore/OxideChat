//! Checks the embedding and rerank models the setup page picks.

use super::{EmbeddingSettings, RagError};
use crate::ai::provider_to_config;
use crate::types::gateway::model_type;
use crate::types::models::Model;
use crate::types::providers::Provider;
use crate::types::{BUDGET_RESERVATION_METADATA_KEY, BudgetError, BudgetSubject, JobState, TokenEstimate};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use omniference::types::{EmbeddingInput, EmbeddingRequestIR, EmbeddingVector, ModelRef};
use sqlx::PgPool;
use sqlx::types::Json;
use std::collections::BTreeMap;
use uuid::Uuid;

const TEST_INPUT: &str = "OxideChat knowledge test";

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
	.fetch_one(pool)
	.await?;
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
	let (model, provider) = load(&state.db, &settings.model_id, "embedding", RagError::NotEmbeddingModel).await?;
	let estimate = TokenEstimate::text(TokenEstimate::input_for_bytes(TEST_INPUT.len() as u64), 0);
	let admission = match state
		.budget_ledger
		.reserve_for_model(&state.db, BudgetSubject::user(*user_id), &model.id, estimate)
		.await
	{
		Ok(admission) => admission,
		Err(BudgetError::Exceeded) => return Err(RagError::BudgetExceeded),
		Err(error) => return Err(RagError::EmbeddingFailed(error.to_string())),
	};
	let mut metadata = BTreeMap::from([
		("oxide_user_id".to_string(), user_id.to_string()),
		("oxide_model_id".to_string(), model.id.to_string()),
		("oxide_request_type".to_string(), "rag_setup".to_string()),
	]);
	if let Some(reservation) = &admission.reservation {
		metadata.insert(BUDGET_RESERVATION_METADATA_KEY.to_string(), reservation.id.to_string());
	}
	let request = EmbeddingRequestIR {
		model: ModelRef {
			alias: model.display_name.clone(),
			provider: provider_to_config(&provider).map_err(|error| RagError::EmbeddingFailed(error.to_string()))?,
			model_id: model.model_id.clone(),
			input_modalities: Vec::new(),
			output_modalities: Vec::new(),
		},
		input: EmbeddingInput::Text(TEST_INPUT.to_string()),
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
	let size = response
		.data
		.first()
		.map(|item| vector_size(&item.embedding))
		.ok_or_else(|| RagError::EmbeddingFailed("no embedding returned".to_string()))?;
	match (u32::try_from(size), settings.dimensions) {
		(Ok(size), Some(requested)) if size != requested => Err(RagError::DimensionsMismatch),
		(Ok(size), _) if (1..=super::MAX_DIMENSIONS).contains(&size) => Ok(size),
		_ => Err(RagError::InvalidDimensions),
	}
}

/// Number of floats in a vector, also when the provider sent it as base64 little-endian `f32`s.
#[must_use]
pub fn vector_size(vector: &EmbeddingVector) -> usize {
	match vector {
		EmbeddingVector::Float(values) => values.len(),
		EmbeddingVector::Base64(encoded) => BASE64.decode(encoded).map_or(0, |bytes| bytes.len() / 4),
	}
}
