#[cfg(test)]
pub mod support {
	use crate::types::*;
	use crate::utils::auth::hash_password;
	use omniference::types::{ModelRef, ProviderConfig, ProviderEndpoint, ProviderKind};
	use sqlx::PgPool;
	use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
	use std::collections::BTreeMap;
	use std::sync::Arc;
	use uuid::Uuid;

	pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

	pub fn request() -> GatewayAdmissionRequest {
		GatewayAdmissionRequest {
			team_id: Uuid::from_u128(1),
			key_id: Uuid::from_u128(2),
			policy: Arc::new(GatewayRatePolicy::default()),
			route_class: GatewayRouteClass::Inference,
			tokens: 0,
		}
	}

	pub fn rule(id: u128, metric: GatewayRuleMetric, capacity: u64, seconds: u32) -> GatewayRateRule {
		GatewayRateRule {
			id: Uuid::from_u128(id),
			route_class: GatewayRouteClass::Inference,
			metric,
			capacity,
			refill_period_seconds: seconds,
		}
	}

	pub fn utility_token_rule() -> GatewayRateRule {
		GatewayRateRule {
			route_class: GatewayRouteClass::Utility,
			..rule(10, GatewayRuleMetric::Tokens, 100, 60)
		}
	}

	pub fn assert_rejected(result: Result<GatewayAdmission, GatewayLimitError>, expected: GatewayLimitDimension) {
		assert!(
			matches!(result, Err(GatewayLimitError::Rejected { dimension, .. }) if dimension == expected),
			"expected a {expected:?} rejection"
		);
	}

	pub fn job_state(db: PgPool) -> Arc<JobState> {
		Arc::new(JobState {
			db,
			gateway_limiter: GatewayLimiter::spawn(),
			budget_ledger: BudgetLedger::spawn(),
			login_limiter: crate::utils::login_limit::LoginRateLimiter::spawn(),
			mcp_pool: crate::utils::tools::McpConnectionPool::new(),
			client_tool_pending: crate::types::state::ClientToolPending::new(),
		})
	}

	pub fn detached_job_state() -> Arc<JobState> {
		job_state(PgPoolOptions::new().connect_lazy_with(PgConnectOptions::new()))
	}

	pub async fn create_user(pool: &PgPool) -> TestResult<Uuid> {
		Ok(
			sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ('gateway@example.com', 'gateway', 'hash') RETURNING id")
				.fetch_one(pool)
				.await?,
		)
	}

	pub async fn create_key(pool: &PgPool, user_id: Uuid, team_id: Uuid, scopes: serde_json::Value) -> TestResult<(Uuid, String)> {
		insert_key(pool, Some(user_id), team_id, scopes).await
	}

	pub async fn create_key_in_new_team(pool: &PgPool, user_id: Uuid, scopes: serde_json::Value) -> TestResult<(Uuid, String)> {
		let team_id = create_team(pool, user_id, &format!("Key Team {}", Uuid::new_v4()), false).await?;
		create_key(pool, user_id, team_id, scopes).await
	}

	pub async fn insert_key(pool: &PgPool, user_id: Option<Uuid>, team_id: Uuid, scopes: serde_json::Value) -> TestResult<(Uuid, String)> {
		let key_id = Uuid::new_v4();
		let secret = "abcdefghijklmnopqrstuvwxyz_0123456789";
		let token = format!("oxc_{}_{}", key_id.simple(), secret);
		sqlx::query(
			"INSERT INTO gateway_api_keys (id, user_id, team_id, name, secret_hash, key_prefix, last_four, scopes)
			 VALUES ($1, $2, $3, 'Test', $4, $5, $6, $7)",
		)
		.bind(key_id)
		.bind(user_id)
		.bind(team_id)
		.bind(hash_password(secret).map_err(|error| std::io::Error::other(error.to_string()))?)
		.bind(format!("oxc_{}", key_id.simple()))
		.bind(&secret[secret.len() - 4..])
		.bind(scopes)
		.execute(pool)
		.await?;
		Ok((key_id, token))
	}

	pub async fn create_team(pool: &PgPool, user_id: Uuid, name: &str, allow_all_models: bool) -> TestResult<Uuid> {
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

	pub async fn create_provider_model(pool: &PgPool, provider_name: &str, model_name: &str) -> TestResult<(Uuid, Uuid)> {
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

	pub fn model_ref() -> ModelRef {
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
}

#[cfg(test)]
mod limiter {
	use super::support::*;
	use crate::types::*;
	use crate::utils::omniference_cost::OxideCostQueue;
	use ::axum::body::{Body, Bytes, to_bytes};
	use ::axum::response::Response;
	use futures_util::StreamExt;
	use http_body::Body as _;
	use omniference::catalog::UsageBreakdown;
	use omniference::middleware::cost::{CostFinalization, CostSink};
	use std::collections::BTreeMap;
	use std::pin::Pin;
	use std::sync::Arc;
	use std::time::Duration;
	use uuid::Uuid;

	#[tokio::test(start_paused = true)]
	async fn terminal_usage_reconciles_every_finalization_once() -> TestResult {
		let state = detached_job_state();
		let queue = OxideCostQueue::spawn(Arc::clone(&state));
		for finalization in [
			CostFinalization::ProviderReported,
			CostFinalization::Done,
			CostFinalization::Error,
			CostFinalization::EndOfStream,
			CostFinalization::Dropped,
		] {
			let mut request = request();
			request.team_id = Uuid::new_v4();
			request.tokens = 90;
			Arc::make_mut(&mut request.policy).team_rules.push(rule(10, GatewayRuleMetric::Tokens, 100, 60));
			let admission = state.gateway_limiter.admit(request.clone()).await?;
			let metadata = BTreeMap::from([("oxide_reservation_id".to_string(), admission.reservation.id.to_string())]);
			let usage = UsageBreakdown {
				input_tokens: 5,
				output_tokens: 10,
				reasoning_tokens: 15,
				..Default::default()
			};
			drop(admission);
			queue.terminal_usage(finalization, &metadata, &usage);
			queue.terminal_usage(finalization, &metadata, &UsageBreakdown::default());
			request.tokens = 80;
			let next = state.gateway_limiter.admit(request).await?;
			assert_eq!(next.snapshot.tokens.as_ref().map(|snapshot| snapshot.remaining), Some(0));
		}
		queue.shutdown().await;
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn team_and_key_request_rules_are_atomic() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_rules.push(rule(10, GatewayRuleMetric::Requests, 2, 60));
		Arc::make_mut(&mut request.policy).key_rules.push(rule(11, GatewayRuleMetric::Requests, 1, 60));
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Requests);
		request.key_id = Uuid::from_u128(3);
		drop(limiter.admit(request.clone()).await?);
		request.key_id = Uuid::from_u128(4);
		assert_rejected(limiter.admit(request).await, GatewayLimitDimension::Requests);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn team_wins_over_permissive_key() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_rules.push(rule(10, GatewayRuleMetric::Requests, 1, 60));
		Arc::make_mut(&mut request.policy).key_rules.push(rule(11, GatewayRuleMetric::Requests, 100, 60));
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request).await, GatewayLimitDimension::Requests);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn overlapping_request_windows_and_maximum_retry() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_rules = vec![rule(10, GatewayRuleMetric::Requests, 1, 10), rule(11, GatewayRuleMetric::Requests, 2, 60)];
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Requests);
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
		Arc::make_mut(&mut request.policy).team_rules = vec![
			rule(10, GatewayRuleMetric::Tokens, 1_000_000, 120),
			rule(11, GatewayRuleMetric::Tokens, 2_000_000, 300),
		];
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Tokens);
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
		assert_rejected(limiter.admit(request).await, GatewayLimitDimension::Tokens);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn reconciliation_refunds_every_rule_once_and_charges_excess() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 80;
		Arc::make_mut(&mut request.policy).team_rules = vec![rule(10, GatewayRuleMetric::Tokens, 100, 60), rule(11, GatewayRuleMetric::Tokens, 200, 300)];
		Arc::make_mut(&mut request.policy).key_rules = vec![rule(12, GatewayRuleMetric::Tokens, 100, 60)];
		let first = limiter.admit(request.clone()).await?;
		limiter.reconcile(first.reservation.id, 20);
		limiter.reconcile(first.reservation.id, 0);
		drop(first);
		let second = limiter.admit(request.clone()).await?;
		assert_eq!(second.snapshot.tokens.as_ref().map(|snapshot| snapshot.remaining), Some(0));
		limiter.reconcile(second.reservation.id, 120);
		drop(second);
		request.tokens = 1;
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Tokens);
		tokio::time::advance(Duration::from_secs(25)).await;
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn team_and_key_concurrency_are_composed() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_concurrency = Some(2);
		Arc::make_mut(&mut request.policy).key_concurrency = Some(1);
		let first = limiter.admit(request.clone()).await?;
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		request.key_id = Uuid::from_u128(3);
		let second = limiter.admit(request.clone()).await?;
		request.key_id = Uuid::from_u128(4);
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		drop(first);
		drop(limiter.admit(request).await?);
		drop(second);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn nonstreaming_and_body_error_release() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let response = reservation.attach(Response::new(Body::from("complete")));
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
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
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"a")), Ok(Bytes::from_static(b"b"))]);
		let mut body = reservation.attach(Response::new(Body::from_stream(stream))).into_body();
		assert!(futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await.is_some());
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		to_bytes(body, 100).await?;
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn unpolled_stream_drop_and_serving_task_cancellation_release() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let stream = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
		let response = reservation.attach(Response::new(Body::from_stream(stream)));
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		drop(response);
		let reservation = limiter.admit(request.clone()).await?.reservation;
		let response = reservation.attach(Response::new(Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>())));
		let (ready, started) = tokio::sync::oneshot::channel();
		let task = tokio::spawn(async move {
			let _ = ready.send(());
			let _ = to_bytes(response.into_body(), 100).await;
		});
		started.await?;
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		task.abort();
		assert!(task.await.is_err());
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn utility_is_independent_and_never_reserves_tokens_or_concurrency() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		Arc::make_mut(&mut request.policy).team_rules = vec![rule(10, GatewayRuleMetric::Requests, 1, 60), rule(11, GatewayRuleMetric::Tokens, 1, 60)];
		let inference = limiter.admit(request.clone()).await?;
		request.route_class = GatewayRouteClass::Utility;
		request.tokens = u64::MAX;
		let utility = limiter.admit(request.clone()).await?;
		assert!(utility.snapshot.requests.is_none() && utility.snapshot.tokens.is_none() && utility.snapshot.concurrency.is_none());
		let mut utility_rule = rule(12, GatewayRuleMetric::Requests, 1, 60);
		utility_rule.route_class = GatewayRouteClass::Utility;
		Arc::make_mut(&mut request.policy).team_rules.push(utility_rule);
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request).await, GatewayLimitDimension::Requests);
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
		Arc::make_mut(&mut request.policy).team_rules.push(rule(10, GatewayRuleMetric::Tokens, 100, 60));
		drop(limiter.admit(request).await?);
		assert_eq!(limiter.state_counts().await?.buckets, 1);
		tokio::time::advance(Duration::from_secs(121)).await;
		tokio::task::yield_now().await;
		assert_eq!(
			limiter.state_counts().await?,
			GatewayStateCounts {
				buckets: 0,
				concurrency: 0,
				reservations: 0
			}
		);
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn edits_clamp_without_refilling_unrelated_buckets() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		Arc::make_mut(&mut request.policy).team_rules = vec![rule(10, GatewayRuleMetric::Requests, 2, 60), rule(11, GatewayRuleMetric::Requests, 10, 300)];
		drop(limiter.admit(request.clone()).await?);
		Arc::make_mut(&mut request.policy).team_rules[0].capacity = 100;
		drop(limiter.admit(request.clone()).await?);
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Requests);
		Arc::make_mut(&mut request.policy).team_rules.remove(0);
		let admission = limiter.admit(request).await?;
		assert_eq!(admission.snapshot.requests.map(|value| value.remaining), Some(7));
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn active_streams_survive_eviction_and_partial_body_drop_releases() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 10;
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		Arc::make_mut(&mut request.policy).team_rules.push(rule(10, GatewayRuleMetric::Tokens, 100, 60));
		let admission = limiter.admit(request.clone()).await?;
		let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(b"partial"))]).chain(futures_util::stream::pending());
		let mut body = admission.reservation.attach(Response::new(Body::from_stream(stream))).into_body();
		assert!(futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await.is_some());
		tokio::time::advance(Duration::from_secs(121)).await;
		assert_rejected(limiter.admit(request.clone()).await, GatewayLimitDimension::Concurrency);
		assert_eq!(limiter.state_counts().await?.reservations, 1);
		drop(body);
		drop(limiter.admit(request).await?);
		Ok(())
	}

	#[tokio::test]
	async fn unavailable_actor_fails_closed_only_for_configured_limits() -> TestResult {
		let limiter = GatewayLimiter::unavailable();
		let mut request = request();
		drop(limiter.admit(request.clone()).await?);
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		assert!(matches!(limiter.admit(request).await, Err(GatewayLimitError::Unavailable)));
		Ok(())
	}

	#[tokio::test]
	async fn full_admission_queue_overloads_only_limited_routes() -> TestResult {
		let limiter = GatewayLimiter::saturated();
		let mut request = request();
		drop(limiter.admit(request.clone()).await?);
		Arc::make_mut(&mut request.policy).team_concurrency = Some(1);
		assert!(matches!(limiter.admit(request).await, Err(GatewayLimitError::Overloaded)));
		Ok(())
	}
}

#[cfg(test)]
mod policy {
	use super::support::*;
	use crate::types::*;
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	#[test]
	fn malformed_signed_policy_fails_closed() {
		let value = json!({"id": Uuid::nil(), "route_class": "inference", "metric": "tokens", "capacity": -1, "refill_period_seconds": 60});
		assert!(serde_json::from_value::<GatewayRateRule>(value).is_err());
	}

	#[test]
	fn utility_token_rules_fail_closed_for_teams_and_keys() -> TestResult {
		let team = GatewayRatePolicy::from_stored(vec![utility_token_rule()], Vec::new(), None, None);
		let key = GatewayRatePolicy::from_stored(Vec::new(), vec![utility_token_rule()], None, None);
		assert!(matches!(team, Err(GatewayLimitError::Misconfigured)));
		assert!(matches!(key, Err(GatewayLimitError::Misconfigured)));
		Ok(())
	}

	#[test]
	fn invalid_stored_concurrency_is_misconfigured() -> TestResult {
		for (team, key) in [(Some(-1), None), (None, Some(-1)), (Some(0), None), (None, Some(0))] {
			let policy = GatewayRatePolicy::from_stored(Vec::new(), Vec::new(), team, key);
			assert!(matches!(policy, Err(GatewayLimitError::Misconfigured)));
		}
		Ok(())
	}

	#[test]
	fn valid_stored_policy_is_loaded() -> TestResult {
		let rules = vec![rule(10, GatewayRuleMetric::Tokens, 100, 60)];
		let policy = GatewayRatePolicy::from_stored(rules, Vec::new(), Some(3), None)?;
		assert_eq!(policy.team_concurrency, Some(3));
		assert_eq!(policy.key_concurrency, None);
		assert!(policy.has_token_rules());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn database_rejects_utility_token_rules(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		for (team_id, api_key_id) in [(Some(context.team_id), None), (None, Some(key_id))] {
			let result = sqlx::query(
				"INSERT INTO gateway_rate_limit_rules (team_id, api_key_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, $2, 'utility', 'tokens', 100, 60)",
			)
			.bind(team_id)
			.bind(api_key_id)
			.execute(&pool)
			.await;
			let error = result.expect_err("utility token rules must violate the policy constraint");
			assert_eq!(error.as_database_error().and_then(|error| error.constraint()), Some("gateway_utility_requests_only"));
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn database_rejects_duplicate_rule_windows(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (_, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let insert = "INSERT INTO gateway_rate_limit_rules (team_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'tokens', 7, 120)";
		sqlx::query(insert).bind(context.team_id).execute(&pool).await?;
		let result = sqlx::query(insert).bind(context.team_id).execute(&pool).await;
		let error = result.expect_err("a duplicate window must violate the unique index");
		assert_eq!(
			error.as_database_error().and_then(|error| error.constraint()),
			Some("idx_gateway_rate_limit_rules_team_window")
		);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_and_key_policies_load_together_and_disabled_rules_are_ignored(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let initial = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(!initial.policy.applies(GatewayRouteClass::Inference));
		sqlx::query("UPDATE teams SET gateway_max_concurrent_requests = 3 WHERE id = $1")
			.bind(initial.team_id)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE gateway_api_keys SET max_concurrent_requests = 2 WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (team_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'tokens', 1000000, 120), ($1, 'inference', 'tokens', 2000000, 300)")
			.bind(initial.team_id).execute(&pool).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds, is_enabled) VALUES ($1, 'utility', 'requests', 10, 60, true), ($1, 'inference', 'requests', 1, 60, false)")
			.bind(key_id).execute(&pool).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.policy.team_concurrency, Some(3));
		assert_eq!(context.policy.key_concurrency, Some(2));
		assert_eq!(context.policy.team_rules.len(), 2);
		assert_eq!(context.policy.key_rules.len(), 1);
		assert_eq!(context.policy.key_rules[0].route_class, GatewayRouteClass::Utility);
		Ok(())
	}
}

#[cfg(test)]
mod estimate {
	use super::support::*;
	use crate::types::*;
	use uuid::Uuid;

	#[test]
	fn token_estimation_uses_output_and_external_input_bounds() -> TestResult {
		let inference = GatewayInference {
			model_id: Uuid::nil(),
			engine_model_id: String::new(),
			public_model_id: String::new(),
			provider_name: String::new(),
			max_output_tokens: Some(100),
			context_length: Some(1000),
			budget: BudgetAdmission::default(),
		};
		let text = serde_json::to_vec(&serde_json::json!({"messages": [{"role": "user", "content": "hello"}]}))?;
		let external = serde_json::to_vec(&serde_json::json!({"previous_response_id": "opaque-history"}))?;
		let default = inference.estimate_tokens(&text, None, 1)?;
		let explicit = inference.estimate_tokens(&text, Some(200), 1)?;
		assert_eq!(explicit, default + 100);
		assert_eq!(inference.estimate_tokens(&text, None, 2)?, default * 2);
		assert_eq!(inference.estimate_tokens(&external, None, 1)?, 1100);
		let unknown = GatewayInference {
			model_id: Uuid::nil(),
			engine_model_id: String::new(),
			public_model_id: String::new(),
			provider_name: String::new(),
			max_output_tokens: None,
			context_length: None,
			budget: BudgetAdmission::default(),
		};
		assert!(unknown.estimate_tokens(&text, None, 1).is_err());
		assert!(unknown.estimate_tokens(&external, Some(100), 1).is_err());
		Ok(())
	}
}

#[cfg(test)]
mod authentication {
	use super::support::*;
	use crate::types::*;
	use serde_json::json;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn bearer_key_authenticates_and_updates_last_used(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read", "inference:write"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.key_id, key_id);
		assert_eq!(context.user_id, Some(user_id));
		assert!(context.allows("inference:write"));
		let last_used: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT last_used_at FROM gateway_api_keys WHERE id = $1")
			.bind(key_id)
			.fetch_one(&pool)
			.await?;
		assert!(last_used.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn disabled_keys_fail_authentication(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		sqlx::query("UPDATE gateway_api_keys SET is_enabled = false WHERE id = $1")
			.bind(key_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_whose_user_left_the_team_is_rejected(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Leaving Team", false).await?;
		let (_, token) = create_key(&pool, user_id, team_id, json!(["inference:read"])).await?;
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
			.bind(team_id)
			.bind(user_id)
			.execute(&pool)
			.await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_without_user_authenticates_as_a_service_account(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Service Team", false).await?;
		let (_, token) = insert_key(&pool, None, team_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.user_id, None);
		assert_eq!(context.team_id, team_id);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_rules_are_shared_by_keys_of_the_same_team_only(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Limited Team", false).await?;
		let other_team_id = create_team(&pool, user_id, "Unlimited Team", false).await?;
		let (_, first_token) = create_key(&pool, user_id, team_id, json!(["inference:read"])).await?;
		let (_, second_token) = create_key(&pool, user_id, team_id, json!(["inference:read"])).await?;
		let (_, other_token) = create_key(&pool, user_id, other_team_id, json!(["inference:read"])).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (team_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'requests', 5, 60)")
			.bind(team_id)
			.execute(&pool)
			.await?;
		let first = GatewayCredential::authenticate(&pool, &first_token).await?;
		let second = GatewayCredential::authenticate(&pool, &second_token).await?;
		let other = GatewayCredential::authenticate(&pool, &other_token).await?;
		assert_eq!(first.team_id, second.team_id);
		assert_eq!(first.policy.team_rules.len(), 1);
		assert_eq!(second.policy.team_rules.len(), 1);
		assert_eq!(first.policy.team_rules[0].id, second.policy.team_rules[0].id);
		assert_ne!(first.team_id, other.team_id);
		assert!(other.policy.team_rules.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_revoked_and_expired_keys_are_rejected(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:write"])).await?;
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
}

#[cfg(test)]
mod models {
	use super::support::*;
	use crate::types::*;
	use serde_json::json;
	use sqlx::PgPool;

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
		let (_, token) = create_key(&pool, user_id, team_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].id, "gateway provider/visible");
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/visible").await?.is_some());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gateway provider/hidden").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn provider_prefixed_stored_ids_are_listed_once_and_resolve_in_both_forms(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Prefixed Team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "OpenAI", "openai/gpt-4.1-nano").await?;
		let (_, token) = create_key(&pool, user_id, team_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert_eq!(models[0].id, "openai/gpt-4.1-nano");
		for requested in ["openai/gpt-4.1-nano", "OpenAI/openai/gpt-4.1-nano", "openai/openai/gpt-4.1-nano"] {
			assert_eq!(GatewayModel::resolve_accessible(&pool, &context, requested).await?, Some(model_id), "{requested}");
		}
		let inference = GatewayModel::resolve_inference(&pool, &context, "openai/openai/gpt-4.1-nano").await?;
		assert_eq!(inference.engine_model_id, "openai/gpt-4.1-nano");
		assert!(GatewayModel::resolve_accessible(&pool, &context, "gpt-4.1-nano").await?.is_none());
		let teams = GatewayTeamResponse::list_for_user(&pool, &user_id).await?;
		assert_eq!(teams[0].models, vec!["openai/gpt-4.1-nano".to_string()]);
		Ok(())
	}

	#[test]
	fn public_model_ids_add_the_provider_only_when_missing() {
		assert_eq!(public_model_id("OpenAI", "openai/gpt-4.1-nano"), "openai/gpt-4.1-nano");
		assert_eq!(public_model_id("Gateway Provider", "visible"), "gateway provider/visible");
		assert_eq!(public_model_id("OpenRouter", "openai/gpt-4o"), "openrouter/openai/gpt-4o");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_only_sees_models_of_its_pinned_team(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let first_team = create_team(&pool, user_id, "First Gateway Team", false).await?;
		let second_team = create_team(&pool, user_id, "Second Gateway Team", false).await?;
		let (provider_id, shared_id) = create_provider_model(&pool, "Shared Provider", "shared").await?;
		let other_id: uuid::Uuid = sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'other', 'Other', true) RETURNING id")
			.bind(provider_id)
			.fetch_one(&pool)
			.await?;
		for (team_id, model_id) in [(first_team, shared_id), (second_team, shared_id), (second_team, other_id)] {
			sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)")
				.bind(team_id)
				.bind(model_id)
				.execute(&pool)
				.await?;
		}
		let (_, token) = create_key(&pool, user_id, first_team, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert_eq!(GatewayModel::resolve_accessible(&pool, &context, "shared provider/shared").await?, Some(shared_id));
		assert!(GatewayModel::resolve_accessible(&pool, &context, "shared provider/other").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn paid_inference_allows_concurrent_admission(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Postpaid Team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "Postpaid Provider", "paid").await?;
		let (_, token) = create_key(&pool, user_id, team_id, json!(["inference:write"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let ledger = BudgetLedger::spawn();
		let first = GatewayModel::resolve_inference(&pool, &context, "postpaid provider/paid").await?;
		let first = GatewayModel::authorize_inference(&pool, &ledger, &context, first, TokenEstimate::default()).await?;
		let second = GatewayModel::resolve_inference(&pool, &context, "postpaid provider/paid").await?;
		let second = GatewayModel::authorize_inference(&pool, &ledger, &context, second, TokenEstimate::default()).await?;
		assert_eq!(first.model_id, model_id);
		assert_eq!(second.model_id, model_id);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_only_uses_its_pinned_team_policy(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let pinned_team_id = create_team(&pool, user_id, "Pinned Team", false).await?;
		create_team(&pool, user_id, "Other Team", true).await?;
		create_provider_model(&pool, "Scoped Provider", "scoped").await?;
		let (_, token) = create_key(&pool, user_id, pinned_team_id, json!(["inference:read"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(GatewayModel::list_for_context(&pool, &context).await?.is_empty());
		assert!(GatewayModel::resolve_accessible(&pool, &context, "scoped provider/scoped").await?.is_none());
		Ok(())
	}
}

#[cfg(test)]
mod http {
	use super::support::*;
	use crate::types::*;
	use ::axum::Router;
	use ::axum::body::{Body, to_bytes};
	use ::axum::http::Request;
	use ::axum::response::Response;
	use serde_json::{Value, json};
	use sqlx::PgPool;
	use std::sync::Arc;
	use tower::ServiceExt;

	const MODELS_PATH: &str = "/openai/v1/models";

	async fn router(pool: PgPool) -> TestResult<(Router, Arc<JobState>)> {
		crate::config::Config::init(&pool).await;
		crate::i18n::I18n::init(&pool).await;
		let state = job_state(pool);
		Ok((crate::routes::build_router(Arc::clone(&state)), state))
	}

	async fn send(router: &Router, method: &str, path: &str, token: Option<&str>) -> TestResult<Response> {
		let mut request = Request::builder().method(method).uri(path);
		if let Some(token) = token {
			request = request.header("authorization", format!("Bearer {token}"));
		}
		Ok(router.clone().oneshot(request.body(Body::empty())?).await?)
	}

	async fn json_body(response: Response) -> TestResult<Value> {
		Ok(serde_json::from_slice(&to_bytes(response.into_body(), 10000).await?)?)
	}

	async fn assert_error(response: Response, status: u16) -> TestResult<Value> {
		assert_eq!(response.status().as_u16(), status);
		assert!(response.headers().contains_key("x-request-id"));
		assert!(!response.headers().contains_key("request-id"));
		let value = json_body(response).await?;
		assert!(value["error"]["message"].is_string());
		Ok(value)
	}

	fn assert_request_limit_headers(response: &Response) {
		for header in ["x-ratelimit-limit-requests", "x-ratelimit-remaining-requests", "x-ratelimit-reset-requests"] {
			assert!(response.headers().contains_key(header));
		}
		assert!(!response.headers().contains_key("x-ratelimit-limit-tokens"));
		assert!(!response.headers().contains_key("x-oxide-concurrent-limit"));
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn missing_credentials_return_unauthorized(pool: PgPool) -> TestResult {
		let (router, _) = router(pool).await?;
		let response = send(&router, "GET", MODELS_PATH, None).await?;
		assert_error(response, 401).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn missing_scope_returns_forbidden(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (_, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let (router, _) = router(pool).await?;
		let response = send(&router, "POST", "/openai/v1/chat/completions", Some(&token)).await?;
		assert_error(response, 403).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unknown_route_returns_not_found(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (_, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let (router, _) = router(pool).await?;
		let response = send(&router, "GET", "/openai/v1/absent", Some(&token)).await?;
		assert_error(response, 404).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn utility_request_limit_reports_headers_then_rejects(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (key_id, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'utility', 'requests', 1, 60)")
			.bind(key_id)
			.execute(&pool)
			.await?;
		let (router, _) = router(pool).await?;
		let allowed = send(&router, "GET", MODELS_PATH, Some(&token)).await?;
		assert_eq!(allowed.status().as_u16(), 200);
		assert!(allowed.headers().contains_key("x-request-id"));
		assert_request_limit_headers(&allowed);
		let limited = send(&router, "GET", MODELS_PATH, Some(&token)).await?;
		assert_request_limit_headers(&limited);
		assert!(limited.headers().contains_key("retry-after"));
		let value = assert_error(limited, 429).await?;
		assert_eq!(value["error"]["type"], "rate_limit_error");
		assert_eq!(value["error"]["code"], "requests_limit_exceeded");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unavailable_database_returns_server_error(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let (_, token) = create_key_in_new_team(&pool, user_id, json!(["inference:read"])).await?;
		let (router, state) = router(pool).await?;
		state.db.close().await;
		let response = send(&router, "GET", MODELS_PATH, Some(&token)).await?;
		assert_error(response, 500).await?;
		Ok(())
	}

	#[tokio::test(start_paused = true)]
	async fn all_success_headers_are_attached_for_the_binding_rules() -> TestResult {
		let limiter = GatewayLimiter::spawn();
		let mut request = request();
		request.tokens = 10;
		Arc::make_mut(&mut request.policy).team_concurrency = Some(2);
		Arc::make_mut(&mut request.policy).team_rules = vec![
			rule(11, GatewayRuleMetric::Requests, 2, 60),
			rule(10, GatewayRuleMetric::Requests, 2, 120),
			rule(12, GatewayRuleMetric::Tokens, 100, 60),
		];
		let admission = limiter.admit(request).await?;
		assert_eq!(admission.snapshot.requests.as_ref().map(|value| value.rule_id), Some(uuid::Uuid::from_u128(10)));
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
mod skins {
	use super::support::*;
	use omniference::skins::{OpenAIChatSkin, OpenAIResponsesSkin, Skin};
	use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIResponsesRequestPayload};
	use serde_json::json;

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

			#[cfg(test)]
			mod pre_auth_limit {
				use crate::utils::openai_gateway::bearer_token_prefix;
				use axum::body::Body;
				use axum::http::Request;

				fn request_with_authorization(value: &str) -> Request<Body> {
					Request::builder().header("authorization", value).body(Body::empty()).unwrap()
				}

				#[test]
				fn extracts_key_prefix_from_oxc_bearer_token() {
					let key_id = uuid::Uuid::new_v4().simple().to_string();
					let request = request_with_authorization(&format!("Bearer oxc_{key_id}_abcdefghijklmnopqrstuvwxyz0123456789"));
					assert_eq!(bearer_token_prefix(&request), Some(format!("oxc_{key_id}")));
				}

				#[test]
				fn ignores_non_oxc_bearer_tokens() {
					let request = request_with_authorization("Bearer sk-abcdefghijklmnopqrstuvwxyz0123456789");
					assert_eq!(bearer_token_prefix(&request), None);
				}

				#[test]
				fn ignores_missing_authorization_header() {
					let request: Request<Body> = Request::builder().body(Body::empty()).unwrap();
					assert_eq!(bearer_token_prefix(&request), None);
				}
			}
