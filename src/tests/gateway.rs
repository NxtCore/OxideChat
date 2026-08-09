#[cfg(test)]
mod tests {
	use crate::types::*;
	use crate::utils::auth::hash_password;
	use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, Skin};
	use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
	use omniference::types::{ModelRef, ProviderConfig, ProviderEndpoint, ProviderKind};
	use serde_json::json;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use uuid::Uuid;

	type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

	async fn create_user(pool: &PgPool) -> TestResult<Uuid> {
		Ok(
			sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ('gateway@example.com', 'gateway', 'hash') RETURNING id")
				.fetch_one(pool)
				.await?,
		)
	}

	async fn create_key(pool: &PgPool, user_id: Uuid, scopes: serde_json::Value) -> TestResult<(Uuid, String)> {
		create_team_key(pool, user_id, None, scopes).await
	}

	async fn create_team_key(pool: &PgPool, user_id: Uuid, team_id: Option<Uuid>, scopes: serde_json::Value) -> TestResult<(Uuid, String)> {
		let project_id: Uuid = sqlx::query_scalar("INSERT INTO gateway_projects (owner_id, team_id, name) VALUES ($1, $2, 'Gateway') RETURNING id")
			.bind(user_id)
			.bind(team_id)
			.fetch_one(pool)
			.await?;
		let key_id = Uuid::new_v4();
		let secret = "abcdefghijklmnopqrstuvwxyz_0123456789";
		let token = format!("oxc_{}_{}", key_id.simple(), secret);
		sqlx::query(
			"INSERT INTO gateway_api_keys (id, project_id, name, secret_hash, key_prefix, last_four, scopes)
			 VALUES ($1, $2, 'Test', $3, $4, $5, $6)",
		)
		.bind(key_id)
		.bind(project_id)
		.bind(hash_password(secret).map_err(|error| std::io::Error::other(error.to_string()))?)
		.bind(format!("oxc_{}", key_id.simple()))
		.bind(&secret[secret.len() - 4..])
		.bind(scopes)
		.execute(pool)
		.await?;
		Ok((key_id, token))
	}

	async fn create_team(pool: &PgPool, user_id: Uuid, name: &str, allow_all_models: bool) -> TestResult<Uuid> {
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ($1, $2) RETURNING id")
			.bind(name)
			.bind(allow_all_models)
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(team_id)
	}

	async fn create_provider_model(pool: &PgPool, provider_name: &str, model_name: &str) -> TestResult<(Uuid, Uuid)> {
		let provider_id: Uuid =
			sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', $1, 'https://example.com', true) RETURNING id")
				.bind(provider_name)
				.fetch_one(pool)
				.await?;
		let model_id: Uuid = sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, $2, $2, true) RETURNING id")
			.bind(provider_id)
			.bind(model_name)
			.fetch_one(pool)
			.await?;
		Ok((provider_id, model_id))
	}

	fn model_ref() -> ModelRef {
		ModelRef {
			alias: "provider/test-model".to_string(),
			provider: ProviderConfig {
				name: "provider".to_string(),
				endpoint: ProviderEndpoint {
					kind: ProviderKind::OpenAICompat,
					base_url: "https://example.com".to_string(),
					api_key: None,
					extra_headers: BTreeMap::new(),
					timeout: None,
				},
				enabled: true,
				catalog_provider_slug: None,
			},
			model_id: "test-model".to_string(),
			input_modalities: Vec::new(),
			output_modalities: Vec::new(),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn bearer_key_authenticates_and_updates_last_used(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:read", "inference:write"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.key_id, key_id);
		assert_eq!(context.user_id, user_id);
		assert!(context.allows("inference:write"));
		let last_used: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT last_used_at FROM gateway_api_keys WHERE id = $1")
			.bind(key_id)
			.fetch_one(&pool)
			.await?;
		assert!(last_used.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_revoked_and_expired_keys_are_rejected(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:write"])).await?;
		assert!(matches!(
			GatewayCredential::authenticate(&pool, "oxc_00000000000000000000000000000000_abcdefghijklmnopqrstuvwxyz012345").await,
			Err(GatewayAuthError::Invalid)
		));
		sqlx::query("UPDATE gateway_api_keys SET revoked_at = NOW() WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		sqlx::query("UPDATE gateway_api_keys SET revoked_at = NULL, expires_at = NOW() - INTERVAL '1 second' WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_listing_respects_team_policy(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Gateway Team", false).await?;
		let (provider_id, visible_id) = create_provider_model(&pool, "Gateway Provider", "visible").await?;
		sqlx::query("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'hidden', 'Hidden', true)")
			.bind(provider_id)
			.execute(&pool)
			.await?;
		sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(visible_id)
			.execute(&pool)
			.await?;
		let (_, token) = create_key(&pool, user_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].id, "gateway provider/visible");
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/visible").await?.is_some());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/hidden").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn duplicate_team_grants_return_one_model(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let first_team = create_team(&pool, user_id, "First Gateway Team", false).await?;
		let second_team = create_team(&pool, user_id, "Second Gateway Team", false).await?;
		let (_, model_id) = create_provider_model(&pool, "Shared Provider", "shared").await?;
		for team_id in [first_team, second_team] {
			sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)")
				.bind(team_id)
				.bind(model_id)
				.execute(&pool)
				.await?;
		}
		let (_, token) = create_key(&pool, user_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert_eq!(GatewayModel::resolve_accessible(&pool, &context, "shared provider/shared").await?, Some(model_id));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn paid_inference_allows_concurrent_admission(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		create_team(&pool, user_id, "Postpaid Team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "Postpaid Provider", "paid").await?;
		let (_, token) = create_key(&pool, user_id, json!(["inference:write"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let first = GatewayModel::authorize_inference(&pool, &context, "postpaid provider/paid").await?;
		let second = GatewayModel::authorize_inference(&pool, &context, "postpaid provider/paid").await?;
		assert_eq!(first.model_id, model_id);
		assert_eq!(second.model_id, model_id);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_project_only_uses_its_team_policy(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let project_team_id = create_team(&pool, user_id, "Project Team", false).await?;
		create_team(&pool, user_id, "Other Team", true).await?;
		create_provider_model(&pool, "Scoped Provider", "scoped").await?;
		let (_, token) = create_team_key(&pool, user_id, Some(project_team_id), json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(GatewayModel::list_for_context(&pool, &context).await?.is_empty());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "scoped provider/scoped").await?.is_none());
		Ok(())
	}

	#[test]
	fn chat_skin_converts_streaming_and_non_streaming_requests() -> TestResult {
		for stream in [false, true] {
			let request: OpenAIChatRequest = serde_json::from_value(json!({
				"model": "test-model",
				"messages": [{"role": "user", "content": "hello"}],
				"stream": stream
			}))?;
			let ir = OpenAIChatSkin::external_to_ir(request, model_ref())?;
			assert_eq!(ir.stream, stream);
			assert_eq!(ir.messages.len(), 1);
			assert!(ir.openai_chat_request.is_some());
		}
		Ok(())
	}

	#[test]
	fn responses_skin_converts_streaming_and_non_streaming_requests() -> TestResult {
		for stream in [false, true] {
			let request: OpenAIResponsesRequestPayload = serde_json::from_value(json!({
				"model": "test-model",
				"input": "hello",
				"stream": stream
			}))?;
			let ir = OpenAIResponsesSkin::external_to_ir(request, model_ref())?;
			assert_eq!(ir.stream, stream);
			assert_eq!(ir.messages.len(), 1);
		}
		Ok(())
	}
}
