#[cfg(test)]
mod support {
	use sqlx::PgPool;
	use uuid::Uuid;

	pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

	pub async fn create_user(pool: &PgPool) -> TestResult<Uuid> {
		let name = format!("user{}", Uuid::new_v4().simple());
		Ok(
			sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ($1, $2, 'hash') RETURNING id")
				.bind(format!("{name}@example.com"))
				.bind(&name)
				.fetch_one(pool)
				.await?,
		)
	}

	pub async fn create_team(pool: &PgPool, user_id: Uuid) -> TestResult<Uuid> {
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ($1, TRUE) RETURNING id")
			.bind(format!("Team {}", Uuid::new_v4()))
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(team_id)
	}

	pub async fn create_key(pool: &PgPool, user_id: Uuid, team_id: Uuid) -> TestResult<Uuid> {
		Ok(
			sqlx::query_scalar(
				r#"
				INSERT INTO gateway_api_keys (user_id, team_id, name, key_prefix, last_four, secret_hash, scopes)
				VALUES ($1, $2, 'Backend', 'oxc_key_', 'abcd', 'hash', '["inference:read","inference:write"]'::jsonb)
				RETURNING id
				"#,
			)
			.bind(user_id)
			.bind(team_id)
			.fetch_one(pool)
			.await?,
		)
	}
}

#[cfg(test)]
mod logs {
	use super::support::{TestResult, create_key, create_team, create_user};
	use crate::types::{
		DEFAULT_REQUEST_LOG_RETENTION_DAYS, RequestLog, RequestLogFilter, RequestLogRecord, RequestLogResponse, RequestLogStatus, axum::PaginatedResponse,
	};
	use chrono::{DateTime, Duration, Utc};
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn success_record(user_id: Uuid, team_id: Option<Uuid>, api_key_id: Option<Uuid>, model: &str) -> RequestLogRecord<'_> {
		RequestLogRecord {
			user_id,
			team_id,
			api_key_id,
			model_id: None,
			public_model_id: model,
			request_type: "gateway.chat",
			status: RequestLogStatus::Success,
			status_code: None,
			input_tokens: 100,
			output_tokens: 40,
			reasoning_tokens: 0,
			cost_total: Decimal::new(1, 2),
			request_id: None,
			provider: None,
			error_category: None,
			streaming: false,
			latency_ms: None,
		}
	}

	fn filter(user_id: Uuid) -> RequestLogFilter {
		RequestLogFilter {
			user_id: Some(user_id),
			team_id: None,
			api_key_id: None,
			status: None,
			error_category: None,
			from: None,
			to: None,
		}
	}

	#[test]
	fn status_parses_known_values_only() {
		assert_eq!(RequestLogStatus::from_stored("success"), Some(RequestLogStatus::Success));
		assert_eq!(RequestLogStatus::from_stored("error"), Some(RequestLogStatus::Error));
		assert_eq!(RequestLogStatus::from_stored("unknown"), None);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn record_and_list_for_owner(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let other_id = create_user(&pool).await?;
		RequestLog::record(&pool, success_record(user_id, None, None, "openai/gpt-4o-mini")).await?;
		RequestLog::record(&pool, success_record(other_id, None, None, "openai/gpt-4o-mini")).await?;
		let PaginatedResponse { has_more, items } = RequestLog::list(&pool, filter(user_id), 1, 50).await?;
		assert!(!has_more);
		assert_eq!(items.len(), 1);
		assert_eq!(items[0].public_model_id, "openai/gpt-4o-mini");
		assert_eq!(items[0].status, "success");
		assert_eq!(items[0].input_tokens, 100);
		assert_eq!(items[0].cost_total, Decimal::new(1, 2));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_filters_team_key_status_and_window(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let key_id = create_key(&pool, user_id, team_id).await?;
		RequestLog::record(&pool, success_record(user_id, Some(team_id), Some(key_id), "openai/gpt-4o-mini")).await?;
		let mut failure = success_record(user_id, None, None, "anthropic/claude-3-5-sonnet");
		failure.status = RequestLogStatus::Error;
		failure.status_code = Some(502);
		RequestLog::record(&pool, failure).await?;
		let mut by_team = filter(user_id);
		by_team.team_id = Some(team_id);
		let mut by_key = filter(user_id);
		by_key.api_key_id = Some(key_id);
		let mut by_status = filter(user_id);
		by_status.status = Some(RequestLogStatus::Error);
		let mut by_success = filter(user_id);
		by_success.status = Some(RequestLogStatus::Success);
		let mut from_future = filter(user_id);
		from_future.from = Some(Utc::now() + Duration::hours(1));
		let mut to_past = filter(user_id);
		to_past.to = Some(Utc::now() - Duration::hours(1));
		assert_eq!(RequestLog::list(&pool, filter(user_id), 1, 50).await?.items.len(), 2);
		assert_eq!(RequestLog::list(&pool, by_team, 1, 50).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_key, 1, 50).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_status, 1, 50).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_success, 1, 50).await?.items.len(), 1);
		assert!(RequestLog::list(&pool, from_future, 1, 50).await?.items.is_empty());
		assert!(RequestLog::list(&pool, to_past, 1, 50).await?.items.is_empty());
		let _: DateTime<Utc> = Utc::now();
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_paginates_with_has_more(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		for _ in 0..3 {
			RequestLog::record(&pool, success_record(user_id, None, None, "openai/gpt-4o-mini")).await?;
		}
		let first = RequestLog::list(&pool, filter(user_id), 1, 2).await?;
		assert!(first.has_more);
		assert_eq!(first.items.len(), 2);
		let second = RequestLog::list(&pool, filter(user_id), 2, 2).await?;
		assert!(!second.has_more);
		assert_eq!(second.items.len(), 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn export_returns_all_matching_rows(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		for _ in 0..5 {
			RequestLog::record(&pool, success_record(user_id, None, None, "openai/gpt-4o-mini")).await?;
		}
		let rows = RequestLog::list_for_export(&pool, filter(user_id)).await?;
		assert_eq!(rows.len(), 5);
		assert!(rows.iter().all(|row: &RequestLogResponse| row.status == "success"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn retention_defaults_and_updates(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		assert_eq!(RequestLog::retention_days(&pool, &user_id).await?, DEFAULT_REQUEST_LOG_RETENTION_DAYS);
		RequestLog::set_retention_days(&pool, &user_id, 7).await?;
		assert_eq!(RequestLog::retention_days(&pool, &user_id).await?, 7);
		RequestLog::set_retention_days(&pool, &user_id, 90).await?;
		assert_eq!(RequestLog::retention_days(&pool, &user_id).await?, 90);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn prune_respects_default_and_user_retention(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let other_id = create_user(&pool).await?;
		RequestLog::record(&pool, success_record(user_id, None, None, "openai/gpt-4o-mini")).await?;
		RequestLog::record(&pool, success_record(other_id, None, None, "openai/gpt-4o-mini")).await?;
		RequestLog::set_retention_days(&pool, &user_id, 7).await?;
		sqlx::query("UPDATE gateway_request_logs SET created_at = NOW() - INTERVAL '10 days' WHERE user_id = $1")
			.bind(user_id)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE gateway_request_logs SET created_at = NOW() - INTERVAL '40 days' WHERE user_id = $1")
			.bind(other_id)
			.execute(&pool)
			.await?;
		assert_eq!(RequestLog::prune(&pool).await?, 2);
		assert!(RequestLog::list(&pool, RequestLogFilter::default(), 1, 50).await?.items.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod categories {
	use super::support::{TestResult, create_key, create_team, create_user};
	use crate::types::{RequestLog, RequestLogErrorCategory, RequestLogFailure, RequestLogFilter, RequestLogOwner, RequestLogRecord, RequestLogStatus, RequestLogTrace};
	use crate::types::RequestLogContext;
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	const ALL: [RequestLogErrorCategory; 6] = [
		RequestLogErrorCategory::Upstream,
		RequestLogErrorCategory::Interrupted,
		RequestLogErrorCategory::Budget,
		RequestLogErrorCategory::RateLimit,
		RequestLogErrorCategory::Auth,
		RequestLogErrorCategory::ModelAccess,
	];

	fn trace(context: RequestLogContext, owner: RequestLogOwner) -> RequestLogTrace<'static> {
		RequestLogTrace {
			context,
			owner,
			request_type: "gateway.chat",
			public_model_id: "provider/model",
			model_id: None,
			provider: Some("provider"),
			streaming: true,
		}
	}

	#[test]
	fn categories_round_trip_and_reject_unknown_values() {
		for category in ALL {
			assert_eq!(RequestLogErrorCategory::from_stored(category.as_str()), Some(category));
		}
		assert_eq!(RequestLogErrorCategory::from_stored("other"), None);
		assert_eq!(RequestLogErrorCategory::from_stored(""), None);
	}

	#[test]
	fn latency_is_never_negative_and_saturates() {
		let started_at = chrono::Utc::now();
		assert_eq!(RequestLogContext::latency_between(started_at, started_at - chrono::Duration::seconds(5)), 0);
		assert_eq!(RequestLogContext::latency_between(started_at, started_at + chrono::Duration::milliseconds(250)), 250);
		assert_eq!(RequestLogContext::latency_between(started_at, started_at + chrono::Duration::days(365 * 100)), i32::MAX);
		assert!(RequestLogContext::parse_started_at("not a number").is_none());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn failure_rows_store_all_new_fields_and_filter_by_category(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let owner = RequestLogOwner { user_id, team_id, api_key_id };
		let context = RequestLogContext::start();
		let row = RequestLog::record_failure(
			&pool,
			&trace(context, owner),
			RequestLogFailure {
				category: RequestLogErrorCategory::RateLimit,
				status_code: Some(429),
			},
		)
		.await?;
		assert_eq!(row.request_id, Some(context.request_id));
		assert_eq!(row.provider.as_deref(), Some("provider"));
		assert_eq!(row.error_category.as_deref(), Some("rate_limit"));
		assert_eq!(row.status, "error");
		assert!(row.streaming);
		assert!(row.latency_ms.is_some_and(|latency| latency >= 0));
		let mut by_category = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		by_category.error_category = Some(RequestLogErrorCategory::RateLimit);
		assert_eq!(RequestLog::list(&pool, by_category, 1, 50).await?.items.len(), 1);
		by_category.error_category = Some(RequestLogErrorCategory::Budget);
		assert!(RequestLog::list(&pool, by_category, 1, 50).await?.items.is_empty());
		let exported = RequestLog::list_for_export(&pool, RequestLogFilter { user_id: Some(user_id), ..RequestLogFilter::default() }).await?;
		assert_eq!(exported[0].request_id, Some(context.request_id));
		assert_eq!(exported[0].error_category.as_deref(), Some("rate_limit"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_repeated_request_id_merges_into_one_row(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let owner = RequestLogOwner { user_id, team_id, api_key_id };
		let context = RequestLogContext::start();
		let first = RequestLogRecord {
			user_id,
			team_id: Some(team_id),
			api_key_id: Some(api_key_id),
			model_id: None,
			public_model_id: "provider/model",
			request_type: "gateway.chat",
			status: RequestLogStatus::Error,
			status_code: None,
			input_tokens: 12,
			output_tokens: 3,
			reasoning_tokens: 0,
			cost_total: Decimal::new(5, 3),
			request_id: Some(context.request_id),
			provider: None,
			error_category: Some(RequestLogErrorCategory::Upstream),
			streaming: false,
			latency_ms: Some(40),
		};
		let stored = RequestLog::record(&pool, first).await?;
		let merged = RequestLog::record_failure(
			&pool,
			&trace(context, owner),
			RequestLogFailure {
				category: RequestLogErrorCategory::Upstream,
				status_code: Some(502),
			},
		)
		.await?;
		assert_eq!(stored.id, merged.id);
		assert_eq!(merged.status_code, Some(502));
		assert_eq!(merged.input_tokens, 12);
		assert_eq!(merged.cost_total, Decimal::new(5, 3));
		assert_eq!(merged.provider.as_deref(), Some("provider"));
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gateway_request_logs").fetch_one(&pool).await?;
		assert_eq!(count, 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rows_without_a_request_id_never_collide(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let mut owner = RequestLogOwner { user_id, team_id, api_key_id };
		for _ in 0..2 {
			let record = RequestLogRecord {
				user_id: owner.user_id,
				team_id: None,
				api_key_id: None,
				model_id: None,
				public_model_id: "provider/model",
				request_type: "gateway.chat",
				status: RequestLogStatus::Success,
				status_code: None,
				input_tokens: 0,
				output_tokens: 0,
				reasoning_tokens: 0,
				cost_total: Decimal::ZERO,
				request_id: None,
				provider: None,
				error_category: None,
				streaming: false,
				latency_ms: None,
			};
			RequestLog::record(&pool, record).await?;
		}
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gateway_request_logs").fetch_one(&pool).await?;
		assert_eq!(count, 2);
		owner.user_id = Uuid::new_v4();
		let failure = RequestLogFailure {
			category: RequestLogErrorCategory::Auth,
			status_code: Some(401),
		};
		assert!(RequestLog::record_failure(&pool, &trace(RequestLogContext::start(), owner), failure).await.is_err());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn negative_latency_is_rejected_by_the_database(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let result = sqlx::query("INSERT INTO gateway_request_logs (user_id, public_model_id, request_type, status, latency_ms) VALUES ($1, 'm', 'gateway.chat', 'error', -1)")
			.bind(user_id)
			.execute(&pool)
			.await;
		assert!(result.is_err());
		Ok(())
	}
}

#[cfg(test)]
mod finalization {
	use crate::tests::gateway::support::{TestResult, create_provider_model, create_team, create_user, insert_key, job_state};
	use crate::types::{REQUEST_ID_METADATA_KEY, RequestLog, RequestLogFilter, RequestLogResponse, STARTED_AT_METADATA_KEY, STREAMING_METADATA_KEY};
	use crate::utils::omniference_cost::OxideCostQueue;
	use omniference::catalog::UsageBreakdown;
	use omniference::middleware::cost::{CostFinalization, CostSink};
	use omniference::stream::CostDetails;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use uuid::Uuid;

	struct Setup {
		user_id: Uuid,
		team_id: Uuid,
		key_id: Uuid,
		model_id: Uuid,
	}

	async fn setup(pool: &PgPool) -> TestResult<Setup> {
		let user_id = create_user(pool).await?;
		let team_id = create_team(pool, user_id, "Finalization Team", true).await?;
		let (_, model_id) = create_provider_model(pool, "Final Provider", "final-model").await?;
		let (key_id, _) = insert_key(pool, Some(user_id), team_id, serde_json::json!(["inference:write"])).await?;
		Ok(Setup {
			user_id,
			team_id,
			key_id,
			model_id,
		})
	}

	fn metadata(setup: &Setup, request_id: Uuid) -> BTreeMap<String, String> {
		BTreeMap::from([
			("oxide_user_id".to_string(), setup.user_id.to_string()),
			("oxide_team_id".to_string(), setup.team_id.to_string()),
			("oxide_api_key_id".to_string(), setup.key_id.to_string()),
			("oxide_model_id".to_string(), setup.model_id.to_string()),
			("oxide_request_type".to_string(), "gateway.chat".to_string()),
			(REQUEST_ID_METADATA_KEY.to_string(), request_id.to_string()),
			(STREAMING_METADATA_KEY.to_string(), "true".to_string()),
			(STARTED_AT_METADATA_KEY.to_string(), (chrono::Utc::now().timestamp_millis() - 1500).to_string()),
		])
	}

	fn cost() -> CostDetails {
		CostDetails {
			total: 0.0,
			prompt: None,
			completion: None,
			reasoning: None,
		}
	}

	fn usage() -> UsageBreakdown {
		UsageBreakdown {
			input_tokens: 7,
			output_tokens: 9,
			..Default::default()
		}
	}

	async fn rows(pool: &PgPool, user_id: Uuid) -> TestResult<Vec<RequestLogResponse>> {
		let filter = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		Ok(RequestLog::list(pool, filter, 1, 50).await?.items)
	}

	async fn finalize(pool: &PgPool, setup: &Setup, finalization: CostFinalization) -> TestResult<Uuid> {
		let request_id = Uuid::new_v4();
		let queue = OxideCostQueue::spawn(job_state(pool.clone()));
		queue.record_with_context("Final Provider", "final-model", &cost(), finalization, &metadata(setup, request_id), &usage());
		queue.shutdown().await;
		Ok(request_id)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn completed_requests_are_logged_as_successes(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		for finalization in [CostFinalization::Done, CostFinalization::ProviderReported] {
			sqlx::query("DELETE FROM gateway_request_logs").execute(&pool).await?;
			let request_id = finalize(&pool, &setup, finalization).await?;
			let logged = rows(&pool, setup.user_id).await?;
			assert_eq!(logged.len(), 1);
			assert_eq!(logged[0].status, "success");
			assert_eq!(logged[0].error_category, None);
			assert_eq!(logged[0].request_id, Some(request_id));
			assert_eq!(logged[0].provider.as_deref(), Some("Final Provider"));
			assert!(logged[0].streaming);
			assert!(logged[0].latency_ms.is_some_and(|latency| latency >= 1500));
			assert_eq!((logged[0].input_tokens, logged[0].output_tokens), (7, 9));
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn dropped_and_unfinished_streams_are_interrupted(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		for finalization in [CostFinalization::Dropped, CostFinalization::EndOfStream] {
			sqlx::query("DELETE FROM gateway_request_logs").execute(&pool).await?;
			finalize(&pool, &setup, finalization).await?;
			let logged = rows(&pool, setup.user_id).await?;
			assert_eq!(logged.len(), 1);
			assert_eq!(logged[0].status, "error");
			assert_eq!(logged[0].error_category.as_deref(), Some("interrupted"));
			assert_eq!(logged[0].status_code, None);
			assert_eq!(logged[0].input_tokens, 7);
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn stream_errors_are_upstream_failures(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		finalize(&pool, &setup, CostFinalization::Error).await?;
		let logged = rows(&pool, setup.user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].status, "error");
		assert_eq!(logged[0].error_category.as_deref(), Some("upstream"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_errored_stream_and_an_upstream_failure_share_one_row(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let request_id = finalize(&pool, &setup, CostFinalization::Error).await?;
		let context = crate::types::RequestLogContext {
			request_id,
			started_at: chrono::Utc::now(),
		};
		let trace = crate::types::RequestLogTrace {
			context,
			owner: crate::types::RequestLogOwner {
				user_id: setup.user_id,
				team_id: setup.team_id,
				api_key_id: setup.key_id,
			},
			request_type: "gateway.chat",
			public_model_id: "final provider/final-model",
			model_id: Some(setup.model_id),
			provider: Some("Final Provider"),
			streaming: true,
		};
		let failure = crate::types::RequestLogFailure {
			category: crate::types::RequestLogErrorCategory::Upstream,
			status_code: Some(502),
		};
		RequestLog::record_failure(&pool, &trace, failure).await?;
		let logged = rows(&pool, setup.user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].status_code, Some(502));
		assert_eq!(logged[0].input_tokens, 7);
		Ok(())
	}
}

#[cfg(test)]
mod http {
	use crate::tests::gateway::support::{TestResult, create_key, create_provider_model, create_team, create_user, insert_key, job_state};
	use crate::types::{JobState, RequestLog, RequestLogFilter, RequestLogResponse};
	use ::axum::Router;
	use ::axum::body::Body;
	use ::axum::http::Request;
	use ::axum::response::Response;
	use serde_json::{Value, json};
	use sqlx::PgPool;
	use std::sync::Arc;
	use tower::ServiceExt;
	use uuid::Uuid;

	async fn router(pool: PgPool) -> (Router, Arc<JobState>) {
		crate::config::Config::init(&pool).await;
		crate::i18n::I18n::init(&pool).await;
		let state = job_state(pool);
		(crate::routes::build_router(Arc::clone(&state)), state)
	}

	async fn post(router: &Router, path: &str, token: &str, body: Value) -> TestResult<Response> {
		let request = Request::builder()
			.method("POST")
			.uri(path)
			.header("authorization", format!("Bearer {token}"))
			.header("content-type", "application/json")
			.body(Body::from(serde_json::to_vec(&body)?))?;
		Ok(router.clone().oneshot(request).await?)
	}

	fn chat(model: &str, stream: bool) -> Value {
		json!({"model": model, "messages": [{"role": "user", "content": "hello"}], "max_tokens": 50, "stream": stream})
	}

	fn header_id(response: &Response) -> TestResult<Uuid> {
		Ok(Uuid::parse_str(response.headers().get("x-request-id").ok_or("missing x-request-id")?.to_str()?)?)
	}

	async fn rows(pool: &PgPool, user_id: Uuid) -> TestResult<Vec<RequestLogResponse>> {
		let filter = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		Ok(RequestLog::list(pool, filter, 1, 50).await?.items)
	}

	async fn total_rows(pool: &PgPool) -> TestResult<i64> {
		Ok(sqlx::query_scalar("SELECT COUNT(*) FROM gateway_request_logs").fetch_one(pool).await?)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unknown_model_row_carries_the_response_request_id(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Access Team", true).await?;
		let (key_id, token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		let (router, _) = router(pool.clone()).await;
		let response = post(&router, "/openai/v1/chat/completions", &token, chat("nowhere/missing", true)).await?;
		assert_eq!(response.status().as_u16(), 404);
		let request_id = header_id(&response)?;
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].request_id, Some(request_id));
		assert_eq!(logged[0].error_category.as_deref(), Some("model_access"));
		assert_eq!(logged[0].status, "error");
		assert_eq!(logged[0].status_code, Some(404));
		assert_eq!(logged[0].api_key_id, Some(key_id));
		assert_eq!(logged[0].team_id, Some(team_id));
		assert_eq!(logged[0].request_type, "gateway.chat");
		assert_eq!(logged[0].public_model_id, "nowhere/missing");
		assert!(logged[0].streaming);
		assert!(logged[0].latency_ms.is_some_and(|latency| latency >= 0));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn every_request_gets_its_own_request_id(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Access Team", true).await?;
		let (_, token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		let (router, _) = router(pool.clone()).await;
		let first = post(&router, "/openai/v1/responses", &token, json!({"model": "nowhere/missing", "input": "hello"})).await?;
		let second = post(&router, "/openai/v1/responses", &token, json!({"model": "nowhere/missing", "input": "hello"})).await?;
		let (first_id, second_id) = (header_id(&first)?, header_id(&second)?);
		assert_ne!(first_id, second_id);
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 2);
		assert!(logged.iter().all(|row| row.request_type == "gateway.responses" && !row.streaming));
		assert!(logged.iter().any(|row| row.request_id == Some(first_id)));
		assert!(logged.iter().any(|row| row.request_id == Some(second_id)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn exceeded_budget_is_logged_once_with_model_and_provider(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Budget Team", true).await?;
		let (provider_id, model_id) = create_provider_model(&pool, "Budget Provider", "paid").await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(model_id)
			.bind(json!({"input": 1_000_000.0, "output": 1_000_000.0}))
			.execute(&pool)
			.await?;
		let (key_id, token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		let budget_id: Uuid = sqlx::query_scalar("INSERT INTO budgets (name, amount, kind, interval, on_exceed) VALUES ('Cap', 10::numeric, 'pooled', 'total', 'block') RETURNING id")
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id) VALUES ($1, $2)")
			.bind(budget_id)
			.bind(user_id)
			.execute(&pool)
			.await?;
		sqlx::query("INSERT INTO usage_events (user_id, team_id, api_key_id, model_id, provider_id, request_type, cost_total) VALUES ($1, $2, $3, $4, $5, 'chat', 10)")
			.bind(user_id)
			.bind(team_id)
			.bind(key_id)
			.bind(model_id)
			.bind(provider_id)
			.execute(&pool)
			.await?;
		let (router, _) = router(pool.clone()).await;
		let response = post(&router, "/openai/v1/chat/completions", &token, chat("budget provider/paid", false)).await?;
		assert_eq!(response.status().as_u16(), 429);
		let request_id = header_id(&response)?;
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].error_category.as_deref(), Some("budget"));
		assert_eq!(logged[0].status_code, Some(429));
		assert_eq!(logged[0].request_id, Some(request_id));
		assert_eq!(logged[0].model_id, Some(model_id));
		assert_eq!(logged[0].provider.as_deref(), Some("Budget Provider"));
		assert!(!logged[0].streaming);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rate_limit_rejection_is_logged_once(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Limit Team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "Limit Provider", "limited").await?;
		let (key_id, token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		sqlx::query("INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds) VALUES ($1, 'inference', 'tokens', 1, 60)")
			.bind(key_id)
			.execute(&pool)
			.await?;
		let (router, _) = router(pool.clone()).await;
		let response = post(&router, "/openai/v1/chat/completions", &token, chat("limit provider/limited", true)).await?;
		assert_eq!(response.status().as_u16(), 429);
		assert!(response.headers().contains_key("retry-after"));
		let request_id = header_id(&response)?;
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].error_category.as_deref(), Some("rate_limit"));
		assert_eq!(logged[0].status_code, Some(429));
		assert_eq!(logged[0].request_id, Some(request_id));
		assert_eq!(logged[0].model_id, Some(model_id));
		assert_eq!(logged[0].provider.as_deref(), Some("Limit Provider"));
		assert!(logged[0].streaming);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn missing_scope_and_removed_members_are_logged_as_auth_failures(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Auth Team", true).await?;
		let (read_key, read_token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:read"])).await?;
		let (router, _) = router(pool.clone()).await;
		let response = post(&router, "/openai/v1/chat/completions", &read_token, chat("any/model", false)).await?;
		assert_eq!(response.status().as_u16(), 403);
		let scope_id = header_id(&response)?;
		let (member_key, member_token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2").bind(team_id).bind(user_id).execute(&pool).await?;
		let response = post(&router, "/openai/v1/responses", &member_token, json!({"model": "any/model", "input": "hi"})).await?;
		assert_eq!(response.status().as_u16(), 401);
		let member_id = header_id(&response)?;
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 2);
		assert!(logged.iter().all(|row| row.error_category.as_deref() == Some("auth") && row.status == "error"));
		let scope_row = logged.iter().find(|row| row.request_id == Some(scope_id)).ok_or("missing scope row")?;
		assert_eq!((scope_row.api_key_id, scope_row.status_code, scope_row.request_type.as_str()), (Some(read_key), Some(403), "gateway.chat"));
		let member_row = logged.iter().find(|row| row.request_id == Some(member_id)).ok_or("missing member row")?;
		assert_eq!((member_row.api_key_id, member_row.status_code, member_row.request_type.as_str()), (Some(member_key), Some(401), "gateway.responses"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unknown_and_forged_keys_are_not_logged(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Auth Team", true).await?;
		let (_, token) = create_key(&pool, user_id, team_id, json!(["inference:write"])).await?;
		let (router, _) = router(pool.clone()).await;
		let unknown = format!("oxc_{}_abcdefghijklmnopqrstuvwxyz0123456789", Uuid::new_v4().simple());
		let forged = format!("{}x", &token[..token.len() - 1]);
		for candidate in [unknown.as_str(), forged.as_str(), "not-a-key"] {
			let response = post(&router, "/openai/v1/chat/completions", candidate, chat("any/model", false)).await?;
			assert_eq!(response.status().as_u16(), 401);
		}
		assert_eq!(total_rows(&pool).await?, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn disabled_keys_with_a_valid_secret_are_logged(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Auth Team", true).await?;
		let (key_id, token) = create_key(&pool, user_id, team_id, json!(["inference:write"])).await?;
		sqlx::query("UPDATE gateway_api_keys SET is_enabled = false WHERE id = $1").bind(key_id).execute(&pool).await?;
		let (router, _) = router(pool.clone()).await;
		let response = post(&router, "/openai/v1/chat/completions", &token, chat("any/model", false)).await?;
		assert_eq!(response.status().as_u16(), 401);
		let logged = rows(&pool, user_id).await?;
		assert_eq!(logged.len(), 1);
		assert_eq!(logged[0].api_key_id, Some(key_id));
		assert_eq!(logged[0].error_category.as_deref(), Some("auth"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_listing_is_not_logged(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Auth Team", true).await?;
		let (_, token) = insert_key(&pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		let (router, _) = router(pool.clone()).await;
		let request = Request::builder().method("GET").uri("/openai/v1/models").header("authorization", format!("Bearer {token}")).body(Body::empty())?;
		let response = router.clone().oneshot(request).await?;
		assert_eq!(response.status().as_u16(), 403);
		assert_eq!(total_rows(&pool).await?, 0);
		Ok(())
	}
}
