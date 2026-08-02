#[cfg(test)]
mod tests {
	use crate::types::{GatewayAuthError, GatewayCredential, GatewayModel};
	use crate::utils::auth::hash_password;
	use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, Skin};
	use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
	use omniference::types::{ModelRef, ProviderConfig, ProviderEndpoint, ProviderKind};
	use serde_json::json;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use uuid::Uuid;

	async fn create_user(pool: &PgPool) -> Uuid {
		sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ('gateway@example.com', 'gateway', 'hash') RETURNING id")
			.fetch_one(pool)
			.await
			.unwrap()
	}

	async fn create_key(pool: &PgPool, user_id: Uuid, scopes: serde_json::Value) -> (Uuid, String) {
		create_team_key(pool, user_id, None, scopes).await
	}

	async fn create_team_key(pool: &PgPool, user_id: Uuid, team_id: Option<Uuid>, scopes: serde_json::Value) -> (Uuid, String) {
		let project_id: Uuid = sqlx::query_scalar("INSERT INTO gateway_projects (owner_id, team_id, name) VALUES ($1, $2, 'Gateway') RETURNING id")
			.bind(user_id)
			.bind(team_id)
			.fetch_one(pool)
			.await
			.unwrap();
		let key_id = Uuid::new_v4();
		let secret = "abcdefghijklmnopqrstuvwxyz_0123456789";
		let token = format!("oxc_{}_{}", key_id.simple(), secret);
		sqlx::query(
			"INSERT INTO gateway_api_keys (id, project_id, name, secret_hash, key_prefix, last_four, scopes)
			 VALUES ($1, $2, 'Test', $3, $4, $5, $6)",
		)
		.bind(key_id)
		.bind(project_id)
		.bind(hash_password(secret).unwrap())
		.bind(format!("oxc_{}", key_id.simple()))
		.bind(&secret[secret.len() - 4..])
		.bind(scopes)
		.execute(pool)
		.await
		.unwrap();
		(key_id, token)
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
	async fn bearer_key_authenticates_and_updates_last_used(pool: PgPool) {
		let user_id = create_user(&pool).await;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:read", "inference:write"])).await;
		let context = GatewayCredential::authenticate(&pool, &token).await.unwrap();
		assert_eq!(context.key_id, key_id);
		assert_eq!(context.user_id, user_id);
		assert!(context.allows("inference:write"));
		let last_used: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT last_used_at FROM gateway_api_keys WHERE id = $1")
			.bind(key_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert!(last_used.is_some());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_revoked_and_expired_keys_are_rejected(pool: PgPool) {
		let user_id = create_user(&pool).await;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:write"])).await;
		assert!(matches!(
			GatewayCredential::authenticate(&pool, "oxc_00000000000000000000000000000000_abcdefghijklmnopqrstuvwxyz012345").await,
			Err(GatewayAuthError::Invalid)
		));
		sqlx::query("UPDATE gateway_api_keys SET revoked_at = NOW() WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await
			.unwrap();
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		sqlx::query("UPDATE gateway_api_keys SET revoked_at = NULL, expires_at = NOW() - INTERVAL '1 second' WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await
			.unwrap();
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_listing_respects_team_policy(pool: PgPool) {
		let user_id = create_user(&pool).await;
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ('Gateway Team', false) RETURNING id")
			.fetch_one(&pool)
			.await
			.unwrap();
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(user_id)
			.execute(&pool)
			.await
			.unwrap();
		let provider_id: Uuid = sqlx::query_scalar(
			"INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', 'Gateway Provider', 'https://example.com', true) RETURNING id",
		)
		.fetch_one(&pool)
		.await
		.unwrap();
		let visible_id: Uuid =
			sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'visible', 'Visible', true) RETURNING id")
				.bind(provider_id)
				.fetch_one(&pool)
				.await
				.unwrap();
		sqlx::query("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'hidden', 'Hidden', true)")
			.bind(provider_id)
			.execute(&pool)
			.await
			.unwrap();
		sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(visible_id)
			.execute(&pool)
			.await
			.unwrap();
		let (_, token) = create_key(&pool, user_id, json!(["inference:read"])).await;
		let context = GatewayCredential::authenticate(&pool, &token).await.unwrap();
		let models = GatewayModel::list_for_context(&pool, &context).await.unwrap();
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].id, "gateway provider/visible");
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/visible").await.unwrap().is_some());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/hidden").await.unwrap().is_none());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_project_only_uses_its_team_policy(pool: PgPool) {
		let user_id = create_user(&pool).await;
		let project_team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ('Project Team', false) RETURNING id")
			.fetch_one(&pool)
			.await
			.unwrap();
		let other_team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ('Other Team', true) RETURNING id")
			.fetch_one(&pool)
			.await
			.unwrap();
		for team_id in [project_team_id, other_team_id] {
			sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
				.bind(team_id)
				.bind(user_id)
				.execute(&pool)
				.await
				.unwrap();
		}
		let provider_id: Uuid =
			sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', 'Scoped Provider', 'https://example.com', true) RETURNING id")
				.fetch_one(&pool)
				.await
				.unwrap();
		sqlx::query("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'scoped', 'Scoped', true)")
			.bind(provider_id)
			.execute(&pool)
			.await
			.unwrap();
		let (_, token) = create_team_key(&pool, user_id, Some(project_team_id), json!(["inference:read"])).await;
		let context = GatewayCredential::authenticate(&pool, &token).await.unwrap();
		assert!(GatewayModel::list_for_context(&pool, &context).await.unwrap().is_empty());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "scoped provider/scoped").await.unwrap().is_none());
	}

	#[test]
	fn chat_skin_converts_streaming_and_non_streaming_requests() {
		for stream in [false, true] {
			let request: OpenAIChatRequest = serde_json::from_value(json!({
				"model": "test-model",
				"messages": [{"role": "user", "content": "hello"}],
				"stream": stream
			}))
			.unwrap();
			let ir = OpenAIChatSkin::external_to_ir(request, model_ref()).unwrap();
			assert_eq!(ir.stream, stream);
			assert_eq!(ir.messages.len(), 1);
			assert!(ir.openai_chat_request.is_some());
		}
	}

	#[test]
	fn responses_skin_converts_streaming_and_non_streaming_requests() {
		for stream in [false, true] {
			let request: OpenAIResponsesRequestPayload = serde_json::from_value(json!({
				"model": "test-model",
				"input": "hello",
				"stream": stream
			}))
			.unwrap();
			let ir = OpenAIResponsesSkin::external_to_ir(request, model_ref()).unwrap();
			assert_eq!(ir.stream, stream);
			assert_eq!(ir.messages.len(), 1);
		}
	}
}
