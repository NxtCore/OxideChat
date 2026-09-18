#[cfg(test)]
mod limits {
	use crate::types::gateway::limiter::{GatewayLimitBackend, GatewayLimiter};
	use crate::types::*;
	use axum::body::{Body, Bytes, HttpBody, to_bytes};
	use axum::response::Response;
	use futures_util::StreamExt;
	use std::pin::Pin;
	use std::time::Duration;
	use uuid::Uuid;

	type TestResult = Result<(), Box<dyn std::error::Error>>;

	fn request() -> GatewayAdmissionRequest {
		GatewayAdmissionRequest {
			project_id: Uuid::from_u128(1),
			key_id: Uuid::from_u128(2),
			policy: GatewayRatePolicy::default(),
			route_class: GatewayRouteClass::Inference,
			tokens: 0,
		}
	}

	fn rule(id: u128, metric: GatewayLimitDimension, capacity: u64, seconds: u32) -> GatewayRateRule {
		GatewayRateRule {
			id: Uuid::from_u128(id),
			route_class: GatewayRouteClass::Inference,
			metric,
			capacity,
			refill_period_seconds: seconds,
		}
	}

	#[tokio::test(start_paused = true)]
	async fn project_and_key_request_rules_are_atomic() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_rules.push(rule(10, GatewayLimitDimension::Requests, 2, 60));
		request.policy.key_rules.push(rule(11, GatewayLimitDimension::Requests, 1, 60));
		drop(limiter.admit(request.clone()).await?);
		assert!(matches!(
			limiter.admit(request.clone()).await,
			Err(GatewayLimitError::Rejected {
				dimension: GatewayLimitDimension::Requests,
				..
			})
		));
		request.key_id = Uuid::from_u128(3);
		drop(limiter.admit(request.clone()).await?);
		request.key_id = Uuid::from_u128(4);
		assert!(matches!(
			limiter.admit(request).await,
			Err(GatewayLimitError::Rejected {
				dimension: GatewayLimitDimension::Requests,
				..
			})
		));
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn project_wins_over_permissive_key() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_rules.push(rule(10, GatewayLimitDimension::Requests, 1, 60));
		request.policy.key_rules.push(rule(11, GatewayLimitDimension::Requests, 100, 60));
		drop(limiter.admit(request.clone()).await?);
		assert!(limiter.admit(request).await.is_err());
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn overlapping_request_windows_and_maximum_retry() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_rules = vec![rule(10, GatewayLimitDimension::Requests, 1, 10), rule(11, GatewayLimitDimension::Requests, 2, 60)];
		drop(limiter.admit(request.clone()).await?);
		assert!(limiter.admit(request.clone()).await.is_err());
		tokio::time::advance(Duration::from_secs(10)).await;
		drop(limiter.admit(request.clone()).await?);
		let Err(GatewayLimitError::Rejected { retry_after, .. }) = limiter.admit(request.clone()).await else {
			return Err("expected rejection".into());
		};
		assert_eq!(retry_after, Duration::from_secs(20));
		tokio::time::advance(retry_after).await;
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn overlapping_million_token_windows() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 1_000_000;
		request.policy.project_rules = vec![
			rule(10, GatewayLimitDimension::Tokens, 1_000_000, 120),
			rule(11, GatewayLimitDimension::Tokens, 2_000_000, 300),
		];
		drop(limiter.admit(request.clone()).await?);
		assert!(limiter.admit(request.clone()).await.is_err());
		tokio::time::advance(Duration::from_secs(120)).await;
		drop(limiter.admit(request.clone()).await?);
		tokio::time::advance(Duration::from_secs(120)).await;
		drop(limiter.admit(request.clone()).await?);
		tokio::time::advance(Duration::from_secs(120)).await;
		drop(limiter.admit(request.clone()).await?);
		tokio::time::advance(Duration::from_secs(120)).await;
		drop(limiter.admit(request.clone()).await?);
		tokio::time::advance(Duration::from_secs(120)).await;
		drop(limiter.admit(request.clone()).await?);
		tokio::time::advance(Duration::from_secs(120)).await;
		assert!(limiter.admit(request).await.is_err());
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn reconciliation_refunds_every_rule_once_and_charges_excess() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 80;
		request.policy.project_rules = vec![rule(10, GatewayLimitDimension::Tokens, 100, 60), rule(11, GatewayLimitDimension::Tokens, 200, 300)];
		request.policy.key_rules = vec![rule(12, GatewayLimitDimension::Tokens, 100, 60)];
		let first = limiter.admit(request.clone()).await?;
		limiter.reconcile(first.reservation.id, 20);
		limiter.reconcile(first.reservation.id, 0);
		drop(first);
		let second = limiter.admit(request.clone()).await?;
		assert_eq!(second.snapshot.tokens.as_ref().map(|snapshot| snapshot.remaining), Some(0));
		limiter.reconcile(second.reservation.id, 120);
		drop(second);
		request.tokens = 1;
		assert!(limiter.admit(request.clone()).await.is_err());
		tokio::time::advance(Duration::from_secs(25)).await;
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn project_and_key_concurrency_are_composed() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_concurrency = Some(2);
		request.policy.key_concurrency = Some(1);
		let first = limiter.admit(request.clone()).await?;
		assert!(matches!(
			limiter.admit(request.clone()).await,
			Err(GatewayLimitError::Rejected {
				dimension: GatewayLimitDimension::Concurrency,
				..
			})
		));
		request.key_id = Uuid::from_u128(3);
		let second = limiter.admit(request.clone()).await?;
		request.key_id = Uuid::from_u128(4);
		assert!(limiter.admit(request.clone()).await.is_err());
		drop(first);
		drop(limiter.admit(request).await?);
		drop(second);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn nonstreaming_and_body_error_release() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let response = reservation.attach(Response::new(Body::from("complete")));
		assert!(limiter.admit(request.clone()).await.is_err());
		assert_eq!(to_bytes(response.into_body(), 100).await?.as_ref(), b"complete");
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let stream = futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other("provider failure"))]);
		let mut body = reservation.attach(Response::new(Body::from_stream(stream))).into_body();
		let frame = futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await;
		assert!(matches!(frame, Some(Err(_))));
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn streaming_holds_reservation_until_eof() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"a")), Ok(Bytes::from_static(b"b"))]);
		let mut body = reservation.attach(Response::new(Body::from_stream(stream))).into_body();
		assert!(futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await.is_some());
		assert!(limiter.admit(request.clone()).await.is_err());
		to_bytes(body, 100).await?;
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn unpolled_stream_drop_and_serving_task_cancellation_release() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let stream = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
		let response = reservation.attach(Response::new(Body::from_stream(stream)));
		assert!(limiter.admit(request.clone()).await.is_err());
		drop(response);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let response = reservation.attach(Response::new(Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>())));
		let (ready, started) = tokio::sync::oneshot::channel();
		let task = tokio::spawn(async move {
			let _ = ready.send(());
			let _ = to_bytes(response.into_body(), 100).await;
		});
		started.await?;
		assert!(limiter.admit(request.clone()).await.is_err());
		task.abort();
		assert!(task.await.is_err());
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn utility_is_independent_and_never_reserves_tokens_or_concurrency() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_concurrency = Some(1);
		request.policy.project_rules = vec![rule(10, GatewayLimitDimension::Requests, 1, 60), rule(11, GatewayLimitDimension::Tokens, 1, 60)];
		let inference = limiter.admit(request.clone()).await?;
		request.route_class = GatewayRouteClass::Utility;
		request.tokens = u64::MAX;
		let utility = limiter.admit(request.clone()).await?;
		assert!(utility.snapshot.requests.is_none() && utility.snapshot.tokens.is_none() && utility.snapshot.concurrency.is_none());
		let mut utility_rule = rule(12, GatewayLimitDimension::Requests, 1, 60);
		utility_rule.route_class = GatewayRouteClass::Utility;
		request.policy.project_rules.push(utility_rule);
		drop(limiter.admit(request.clone()).await?);
		assert!(limiter.admit(request).await.is_err());
		drop(inference);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn no_limits_and_inactive_state_eviction() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		let first = limiter.admit(request.clone()).await?;
		let second = limiter.admit(request.clone()).await?;
		assert!(first.snapshot.requests.is_none() && second.snapshot.concurrency.is_none());
		drop((first, second));
		request.tokens = 100;
		request.policy.project_rules.push(rule(10, GatewayLimitDimension::Tokens, 100, 60));
		drop(limiter.admit(request).await?);
		assert_eq!(limiter.state_counts().await?.0, 1);
		tokio::time::advance(Duration::from_secs(121)).await;
		tokio::task::yield_now().await;
		assert_eq!(limiter.state_counts().await?, (0, 0, 0));
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn edits_clamp_without_refilling_unrelated_buckets() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.policy.project_rules = vec![rule(10, GatewayLimitDimension::Requests, 2, 60), rule(11, GatewayLimitDimension::Requests, 10, 300)];
		drop(limiter.admit(request.clone()).await?);
		request.policy.project_rules[0].capacity = 100;
		drop(limiter.admit(request.clone()).await?);
		assert!(limiter.admit(request.clone()).await.is_err());
		request.policy.project_rules.remove(0);
		let admission = limiter.admit(request).await?;
		assert_eq!(admission.snapshot.requests.map(|value| value.remaining), Some(7));
		Ok(())
	}

	#[test]
	fn malformed_signed_policy_fails_closed() {
		let value = serde_json::json!({"id": Uuid::nil(), "route_class": "inference", "metric": "tokens", "capacity": -1, "refill_period_seconds": 60});
		assert!(serde_json::from_value::<GatewayRateRule>(value).is_err());
	}

	#[test]
	fn token_estimation_uses_output_and_external_input_bounds() -> TestResult {
		let inference = GatewayInference {
			model_id: Uuid::nil(),
			max_output_tokens: Some(100),
			context_length: Some(1000),
		};
		let text = serde_json::json!({"messages": [{"role": "user", "content": "hello"}]});
		let default = crate::utils::gateway_limits::estimate_tokens(&text, &inference, None, 1)?;
		let explicit = crate::utils::gateway_limits::estimate_tokens(&text, &inference, Some(200), 1)?;
		assert_eq!(explicit, default + 100);
		assert_eq!(crate::utils::gateway_limits::estimate_tokens(&text, &inference, None, 2)?, default * 2);
		let external = serde_json::json!({"previous_response_id": "opaque-history"});
		assert_eq!(crate::utils::gateway_limits::estimate_tokens(&external, &inference, None, 1)?, 1100);
		let unknown = GatewayInference {
			model_id: Uuid::nil(),
			max_output_tokens: None,
			context_length: None,
		};
		assert!(crate::utils::gateway_limits::estimate_tokens(&text, &unknown, None, 1).is_err());
		assert!(crate::utils::gateway_limits::estimate_tokens(&external, &unknown, Some(100), 1).is_err());
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn active_streams_survive_eviction_and_partial_body_drop_releases() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 10;
		request.policy.project_concurrency = Some(1);
		request.policy.project_rules.push(rule(10, GatewayLimitDimension::Tokens, 100, 60));
		let admission = limiter.admit(request.clone()).await?;
		let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"partial"))]).chain(futures_util::stream::pending());
		let mut body = admission.reservation.attach(Response::new(Body::from_stream(stream))).into_body();
		assert!(futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await.is_some());
		tokio::time::advance(Duration::from_secs(121)).await;
		assert!(limiter.admit(request.clone()).await.is_err());
		assert_eq!(limiter.state_counts().await?.2, 1);
		drop(body);
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test]
	async fn unavailable_actor_fails_closed_only_for_configured_limits() -> TestResult {
		let limiter = GatewayLimiter::unavailable();
		let mut request = request();
		drop(limiter.admit(request.clone()).await?);
		request.policy.project_concurrency = Some(1);
		assert!(matches!(limiter.admit(request).await, Err(GatewayLimitError::Unavailable)));
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn binding_ties_are_stable_and_all_success_headers_are_attached() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 10;
		request.policy.project_concurrency = Some(2);
		request.policy.project_rules = vec![
			rule(11, GatewayLimitDimension::Requests, 2, 60),
			rule(10, GatewayLimitDimension::Requests, 2, 120),
			rule(12, GatewayLimitDimension::Tokens, 100, 60),
		];
		let admission = limiter.admit(request).await?;
		assert_eq!(admission.snapshot.requests.as_ref().map(|value| value.rule_id), Some(Uuid::from_u128(10)));
		let response = crate::utils::gateway_limits::finish(admission, Response::new(Body::from("ok")));
		for header in [
			"x-ratelimit-limit-requests",
			"x-ratelimit-remaining-requests",
			"x-ratelimit-reset-requests",
			"x-ratelimit-limit-tokens",
			"x-ratelimit-remaining-tokens",
			"x-ratelimit-reset-tokens",
			"x-oxide-concurrent-limit",
			"x-oxide-concurrent-remaining",
		] {
			assert!(response.headers().contains_key(header));
		}
		assert_eq!(response.headers()["x-ratelimit-remaining-tokens"], "90");
		assert_eq!(response.headers()["x-oxide-concurrent-remaining"], "1");
		assert!(!response.headers().contains_key("retry-after"));
		Ok(())
	}
}

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
	async fn project_and_key_policies_load_together_and_disabled_rules_are_ignored(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:read"])).await?;
		let initial = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(!initial.policy.applies(GatewayRouteClass::Inference));
		sqlx::query("UPDATE gateway_projects SET max_concurrent_requests = 3 WHERE id = $1")
			.bind(initial.project_id)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE gateway_api_keys SET max_concurrent_requests = 2 WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (project_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'tokens', 1000000, 120), ($1, 'inference', 'tokens', 2000000, 300)")
			.bind(initial.project_id).execute(&pool).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds, is_enabled) VALUES ($1, 'utility', 'requests', 10, 60, true), ($1, 'inference', 'requests', 1, 60, false)")
			.bind(key_id).execute(&pool).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.policy.project_concurrency, Some(3));
		assert_eq!(context.policy.key_concurrency, Some(2));
		assert_eq!(context.policy.project_rules.len(), 2);
		assert_eq!(context.policy.key_rules.len(), 1);
		assert_eq!(context.policy.key_rules[0].route_class, GatewayRouteClass::Utility);
		assert!(
			sqlx::query(
				"INSERT INTO gateway_rate_limit_rules (project_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'tokens', 7, 120)"
			)
			.bind(initial.project_id)
			.execute(&pool)
			.await
			.is_err()
		);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn disabled_projects_and_keys_fail_authentication(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:read"])).await?;
		sqlx::query("UPDATE gateway_api_keys SET is_enabled = false WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		sqlx::query("UPDATE gateway_api_keys SET is_enabled = true WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE gateway_projects SET is_enabled = false WHERE id = (SELECT project_id FROM gateway_api_keys WHERE id = $1)")
			.bind(key_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn enabled_namespace_http_contract_matrix(pool: PgPool) -> TestResult {
		use axum::body::{Body, to_bytes};
		use axum::http::Request;
		use tower::ServiceExt;
		crate::config::Config::init(&pool).await;
		crate::i18n::I18n::init(&pool).await;
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key(&pool, user_id, json!(["inference:read"])).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'utility', 'requests', 1, 60)")
			.bind(key_id)
			.execute(&pool)
			.await?;
		let state = std::sync::Arc::new(JobState {
			db: pool,
			gateway_limiter: crate::types::gateway::limiter::GatewayLimiter::spawn(),
			mcp_pool: crate::utils::tools::McpConnectionPool::new(),
			client_tool_pending: crate::types::state::ClientToolPending::new(),
		});
		let router = crate::routes::build_router(std::sync::Arc::clone(&state));
		for (method, path, authenticated, expected) in [
			("GET", "/openai/v1/models", false, 401),
			("POST", "/openai/v1/chat/completions", true, 403),
			("GET", "/openai/v1/absent", true, 404),
			("GET", "/openai/v1/models", true, 200),
			("GET", "/openai/v1/models", true, 429),
			("GET", "/openai/v1/models", true, 500),
		] {
			if expected == 500 {
				state.db.close().await;
			}
			let mut request = Request::builder().method(method).uri(path);
			if authenticated {
				request = request.header("authorization", format!("Bearer {token}"));
			}
			let response = router.clone().oneshot(request.body(Body::empty())?).await?;
			assert_eq!(response.status().as_u16(), expected);
			assert!(response.headers().contains_key("x-request-id"));
			assert!(!response.headers().contains_key("request-id"));
			if expected == 200 || expected == 429 {
				for header in ["x-ratelimit-limit-requests", "x-ratelimit-remaining-requests", "x-ratelimit-reset-requests"] {
					assert!(response.headers().contains_key(header));
				}
				assert!(!response.headers().contains_key("x-ratelimit-limit-tokens"));
				assert!(!response.headers().contains_key("x-oxide-concurrent-limit"));
			}
			if expected == 429 {
				assert!(response.headers().contains_key("retry-after"));
			}
			let value: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 10000).await?)?;
			if expected >= 400 {
				assert!(value["error"]["message"].is_string());
			}
			if expected == 429 {
				assert_eq!(value["error"]["type"], "rate_limit_error");
				assert_eq!(value["error"]["code"], "requests_limit_exceeded");
			}
		}
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
