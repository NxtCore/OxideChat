#[cfg(test)]
mod tests {
	use crate::types::{GatewayAuthError, GatewayCredential, OpenAiModel};
	use crate::utils::auth::hash_password;
	use crate::utils::openai_gateway::bridge_response;
	use axum::body::Bytes;
	use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, Skin};
	use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
	use omniference::types::{ModelRef, ProviderConfig, ProviderEndpoint, ProviderKind};
	use serde_json::json;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use std::convert::Infallible;
	use std::sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	};
	use uuid::Uuid;

	struct DropFlag(Arc<AtomicBool>);

	impl Drop for DropFlag {
		fn drop(&mut self) {
			self.0.store(true, Ordering::SeqCst);
		}
	}

	async fn create_user(pool: &PgPool) -> Uuid {
		sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ('gateway@example.com', 'gateway', 'hash') RETURNING id")
			.fetch_one(pool)
			.await
			.unwrap()
	}

	async fn create_key(pool: &PgPool, user_id: Uuid, scopes: serde_json::Value) -> (Uuid, String) {
		let project_id: Uuid = sqlx::query_scalar("INSERT INTO gateway_projects (owner_id, name) VALUES ($1, 'Gateway') RETURNING id")
			.bind(user_id)
			.fetch_one(pool)
			.await
			.unwrap();
		let key_id = Uuid::new_v4();
		let secret = "abcdefghijklmnopqrstuvwxyz0123456789";
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
		let models = OpenAiModel::list_for_user(&pool, &user_id).await.unwrap();
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].id, "visible");
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

	#[tokio::test]
	async fn dropping_bridged_stream_releases_the_upstream_body() {
		let dropped = Arc::new(AtomicBool::new(false));
		let guard = DropFlag(dropped.clone());
		let stream = futures_util::stream::once(async move {
			let _guard = guard;
			std::future::pending::<Result<Bytes, Infallible>>().await
		});
		let response = axum07::response::Response::new(axum07::body::Body::from_stream(stream));
		let response = bridge_response(response);
		drop(response);
		assert!(dropped.load(Ordering::SeqCst));
	}
}
