//! Optimized provider utility functions

use crate::ai::OF_ENGINE;
use crate::types::{AiModel, AiProvider, ModelCapabilities, SyncProviderResponse};
use crate::utils::encryption::decrypt_api_key;
use omniference::types::{ProviderConfig, ProviderEndpoint};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub async fn sync_provider_models(pool: &PgPool, provider: &AiProvider) -> Result<SyncProviderResponse, String> {
	let api_key = provider.api_key.as_ref().map(|k| decrypt_api_key(k));
	let extra_headers = parse_extra_headers(&provider.extra_headers);

	let config = ProviderConfig {
		name: provider.name.clone(),
		endpoint: ProviderEndpoint {
			kind: provider.kind.to_omni_kind(),
			base_url: provider.base_url.clone(),
			api_key,
			extra_headers,
			timeout: None,
		},
		enabled: true,
	};

	let engine = OF_ENGINE.get().expect("AI engine not initialized").read().await;
	let engine_service = engine.service();
	if engine_service.provider_manager().read().await.get_provider(&config.name).is_none() {
		if let Err(e) = engine_service.register_provider(config.clone()).await {
			return Err(format!("Connection failed: {e}"));
		}
	}

	let all_discovered = match engine_service.discover_models().await {
		Ok(models) => models,
		Err(e) => return Err(format!("Discovery failed: {e}")),
	};

	let provider_name_lower = provider.name.to_lowercase();
	let discovered: Vec<_> = all_discovered.into_iter().filter(|m| m.provider_name == provider_name_lower).collect();

	let existing = sqlx::query_as::<_, AiModel>("SELECT * FROM models WHERE provider_id = $1")
		.bind(provider.id)
		.fetch_all(pool)
		.await
		.unwrap_or_default();

	let existing_map: BTreeMap<_, _> = existing.iter().map(|m| (m.model_id.as_str(), m)).collect();
	let discovered_ids: HashSet<_> = discovered.iter().map(|m| m.id.as_str()).collect();

	// Prepare bulk operations
	let mut to_insert = Vec::new();
	let mut to_update = Vec::new();
	let mut to_delete = Vec::new();

	// Categorize operations
	for model in &discovered {
		let capabilities_json = serde_json::to_value(&model.capabilities).unwrap_or_default();
		let input_modalities_json = serde_json::to_value(&model.input_modalities).unwrap_or_default();
		let output_modalities_json = serde_json::to_value(&model.output_modalities).unwrap_or_default();

		if existing_map.contains_key(model.id.as_str()) {
			to_update.push((
				provider.id,
				model.id.clone(),
				model.name.clone(),
				capabilities_json,
				input_modalities_json,
				output_modalities_json,
				model.context_length.map(|c| c as i32),
				model.max_tokens.map(|c| c as i32),
			));
		} else {
			to_insert.push((
				provider.id,
				model.id.clone(),
				model.name.clone(),
				capabilities_json,
				input_modalities_json,
				output_modalities_json,
				model.context_length.map(|c| c as i32),
				model.max_tokens.map(|c| c as i32),
			));
		}
	}

	for model in &existing {
		if !discovered_ids.contains(model.model_id.as_str()) {
			to_delete.push(model.id);
		}
	}

	// Execute bulk operations in parallel
	let (insert_result, update_result, delete_result) = tokio::join!(bulk_insert(pool, &to_insert), bulk_update(pool, &to_update), bulk_delete(pool, &to_delete));

	let added = insert_result.unwrap_or(0);
	let updated = update_result.unwrap_or(0);
	let removed = delete_result.unwrap_or(0);

	Ok(SyncProviderResponse {
		success: true,
		models_added: added,
		models_updated: updated,
		models_removed: removed,
		message: format!("Synced {} models", discovered.len()),
	})
}

async fn bulk_insert(
	pool: &PgPool,
	items: &[(
		Uuid,
		String,
		String,
		serde_json::Value,
		serde_json::Value,
		serde_json::Value,
		Option<i32>,
		Option<i32>,
	)],
) -> Result<usize, sqlx::Error> {
	if items.is_empty() {
		return Ok(0);
	}

	let mut query_builder = sqlx::QueryBuilder::new(
		"INSERT INTO models (provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, context_length, max_tokens) ",
	);

	query_builder.push_values(items, |mut b, item| {
		b.push_bind(item.0)
			.push_bind(&item.1)
			.push_bind(&item.2)
			.push_bind(&item.3)
			.push_bind(&item.4)
			.push_bind(&item.5)
			.push_bind(item.6)
			.push_bind(item.7);
	});

	query_builder.build().execute(pool).await?;
	Ok(items.len())
}

async fn bulk_update(
	pool: &PgPool,
	items: &[(
		Uuid,
		String,
		String,
		serde_json::Value,
		serde_json::Value,
		serde_json::Value,
		Option<i32>,
		Option<i32>,
	)],
) -> Result<usize, sqlx::Error> {
	if items.is_empty() {
		return Ok(0);
	}

	// Use unnest for efficient bulk updates
	let provider_ids: Vec<Uuid> = items.iter().map(|i| i.0).collect();
	let model_ids: Vec<&str> = items.iter().map(|i| i.1.as_str()).collect();
	let display_names: Vec<&str> = items.iter().map(|i| i.2.as_str()).collect();
	let capabilities: Vec<&serde_json::Value> = items.iter().map(|i| &i.3).collect();
	let input_modalities: Vec<&serde_json::Value> = items.iter().map(|i| &i.4).collect();
	let output_modalities: Vec<&serde_json::Value> = items.iter().map(|i| &i.5).collect();
	let context_lengths: Vec<Option<i32>> = items.iter().map(|i| i.6).collect();
	let max_tokens: Vec<Option<i32>> = items.iter().map(|i| i.7).collect();

	sqlx::query(
		r#"
		UPDATE models AS m SET
			display_name = u.display_name,
			capabilities = u.capabilities,
			input_modalities = u.input_modalities,
			output_modalities = u.output_modalities,
			context_length = u.context_length,
			max_tokens = u.max_tokens,
			updated_at = NOW()
		FROM (
			SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::jsonb[], $5::jsonb[], $6::jsonb[], $7::int[], $8::int[])
			AS t(provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, context_length, max_tokens)
		) AS u
		WHERE m.provider_id = u.provider_id AND m.model_id = u.model_id
		"#,
	)
	.bind(&provider_ids)
	.bind(&model_ids)
	.bind(&display_names)
	.bind(&capabilities)
	.bind(&input_modalities)
	.bind(&output_modalities)
	.bind(&context_lengths)
	.bind(&max_tokens)
	.execute(pool)
	.await?;

	Ok(items.len())
}

async fn bulk_delete(pool: &PgPool, ids: &[Uuid]) -> Result<usize, sqlx::Error> {
	if ids.is_empty() {
		return Ok(0);
	}

	sqlx::query("DELETE FROM models WHERE id = ANY($1)").bind(ids).execute(pool).await?;

	Ok(ids.len())
}

fn parse_extra_headers(value: &serde_json::Value) -> BTreeMap<String, String> {
	if let Some(obj) = value.as_object() {
		obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
	} else {
		BTreeMap::new()
	}
}
