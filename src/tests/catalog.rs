#[cfg(test)]
mod tests {
	use crate::types::catalog::{AvailabilityState, GatewayCatalogModel, GatewayCatalogSyncInput, GatewayProviderOptionSyncInput};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	const GATEWAY: &str = "openrouter";

	async fn create_provider(pool: &PgPool, name: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO providers (kind, name, base_url, is_enabled)
			VALUES ('OPENROUTER', $1, 'https://openrouter.ai/api', true)
			RETURNING id
			"#,
		)
		.bind(name)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	async fn create_model(pool: &PgPool, provider_id: Uuid, model_id: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO models (provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, is_enabled)
			VALUES ($1, $2, $3, '[]'::jsonb, '["text"]'::jsonb, '["text"]'::jsonb, true)
			RETURNING id
			"#,
		)
		.bind(provider_id)
		.bind(model_id)
		.bind(model_id)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	fn catalog_input(id: &str) -> GatewayCatalogSyncInput {
		GatewayCatalogSyncInput {
			gateway_model_id: id.to_string(),
			display_name: id.to_string(),
			raw: json!({"id": id}),
		}
	}

	async fn availability(pool: &PgPool, provider_id: Uuid, gateway_model_id: &str) -> AvailabilityState {
		sqlx::query_scalar::<_, AvailabilityState>("SELECT availability_state FROM gateway_catalog_models WHERE provider_id = $1 AND gateway_model_id = $2")
			.bind(provider_id)
			.bind(gateway_model_id)
			.fetch_one(pool)
			.await
			.unwrap()
	}

	async fn local_link(pool: &PgPool, provider_id: Uuid, gateway_model_id: &str) -> Option<Uuid> {
		sqlx::query_scalar::<_, Option<Uuid>>("SELECT local_model_id FROM gateway_catalog_models WHERE provider_id = $1 AND gateway_model_id = $2")
			.bind(provider_id)
			.bind(gateway_model_id)
			.fetch_one(pool)
			.await
			.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn public_catalog_rows_can_exist_without_local_model(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		let inputs = vec![catalog_input("openai/gpt-4o"), catalog_input("anthropic/claude-3.5-sonnet")];

		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &inputs).await.unwrap();

		// No runnable models exist, so both rows have NULL local_model_id.
		assert_eq!(local_link(&pool, provider_id, "openai/gpt-4o").await, None);
		let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gateway_catalog_models WHERE provider_id = $1")
			.bind(provider_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(count, 2);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn marks_user_unavailable_and_available(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		let inputs = vec![catalog_input("openai/gpt-4o"), catalog_input("secret/private-model")];
		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &inputs).await.unwrap();

		// Only gpt-4o is visible to the key.
		GatewayCatalogModel::mark_availability(&pool, &provider_id, GATEWAY, &["openai/gpt-4o".to_string()])
			.await
			.unwrap();

		assert_eq!(availability(&pool, provider_id, "openai/gpt-4o").await, AvailabilityState::Available);
		assert_eq!(availability(&pool, provider_id, "secret/private-model").await, AvailabilityState::UserUnavailable);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn links_local_model_via_first_slash_prefix_stripping(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		// models.model_id is the provider-prefixed id: "openrouter/openai/gpt-4o".
		create_model(&pool, provider_id, "openrouter/openai/gpt-4o").await;
		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &[catalog_input("openai/gpt-4o")])
			.await
			.unwrap();

		GatewayCatalogModel::refresh_local_model_ids(&pool, &provider_id, GATEWAY, "OpenRouter")
			.await
			.unwrap();

		assert!(local_link(&pool, provider_id, "openai/gpt-4o").await.is_some());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn delete_absent_removes_stale_rows_and_cascades_options(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &[catalog_input("openai/gpt-4o"), catalog_input("old/model")])
			.await
			.unwrap();

		// Attach an option to the soon-to-be-removed row.
		let stale_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gateway_catalog_models WHERE gateway_model_id = 'old/model'")
			.fetch_one(&pool)
			.await
			.unwrap();
		let option = GatewayProviderOptionSyncInput {
			provider_slug: Some("openai".into()),
			provider_name: Some("OpenAI".into()),
			endpoint_name: None,
			status: Some(0.0),
			quantization: None,
			context_length: None,
			max_completion_tokens: None,
			latency: None,
			throughput: None,
			uptime: None,
			price_input: None,
			price_output: None,
			raw: json!({}),
		};
		GatewayCatalogModel::upsert_provider_options(&pool, &stale_id, std::slice::from_ref(&option))
			.await
			.unwrap();

		// New public catalog no longer contains "old/model".
		GatewayCatalogModel::delete_absent(&pool, &provider_id, GATEWAY, &["openai/gpt-4o".to_string()])
			.await
			.unwrap();

		let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gateway_catalog_models WHERE provider_id = $1")
			.bind(provider_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(remaining, 1);

		let orphan_options = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gateway_model_provider_options WHERE catalog_model_id = $1")
			.bind(stale_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(orphan_options, 0, "options should cascade-delete with the catalog row");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn deleting_runnable_model_nulls_local_link_but_keeps_catalog(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		let model_id = create_model(&pool, provider_id, "openrouter/openai/gpt-4o").await;
		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &[catalog_input("openai/gpt-4o")])
			.await
			.unwrap();
		GatewayCatalogModel::refresh_local_model_ids(&pool, &provider_id, GATEWAY, "OpenRouter")
			.await
			.unwrap();
		assert!(local_link(&pool, provider_id, "openai/gpt-4o").await.is_some());

		sqlx::query("DELETE FROM models WHERE id = $1").bind(model_id).execute(&pool).await.unwrap();

		// Catalog row survives (ON DELETE SET NULL), link is cleared.
		assert_eq!(local_link(&pool, provider_id, "openai/gpt-4o").await, None);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn provider_options_for_model_reports_parent_availability(pool: PgPool) {
		let provider_id = create_provider(&pool, "OpenRouter").await;
		let model_id = create_model(&pool, provider_id, "openrouter/openai/gpt-4o").await;
		GatewayCatalogModel::upsert_public_catalog(&pool, &provider_id, GATEWAY, &[catalog_input("openai/gpt-4o")])
			.await
			.unwrap();
		GatewayCatalogModel::mark_availability(&pool, &provider_id, GATEWAY, &["openai/gpt-4o".to_string()])
			.await
			.unwrap();
		GatewayCatalogModel::refresh_local_model_ids(&pool, &provider_id, GATEWAY, "OpenRouter")
			.await
			.unwrap();

		let result = GatewayCatalogModel::provider_options_for_model(&pool, &model_id).await.unwrap();
		assert_eq!(result.availability_state, Some(AvailabilityState::Available));
		assert_eq!(result.gateway_model_id.as_deref(), Some("openai/gpt-4o"));
		assert!(result.options.is_empty());
	}
}
