#[cfg(test)]
mod support {
	use sqlx::PgPool;
	use uuid::Uuid;

	pub use crate::tests::support::{TestResult, create_user};

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
	use crate::tests::support::paged;
	use super::support::{TestResult, create_key, create_team, create_user};
	use crate::types::{
		DEFAULT_REQUEST_LOG_RETENTION_DAYS, RequestLog, RequestLogFilter, RequestLogRecord, RequestLogResponse, RequestLogStatus, Page,
	};
	use chrono::{DateTime, Duration, Utc};
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn success_record(user_id: Uuid, team_id: Option<Uuid>, api_key_id: Option<Uuid>, model: &str) -> RequestLogRecord<'_> {
		RequestLogRecord {
			user_id: Some(user_id),
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
			finish_reason: None,
			time_to_first_token_ms: None,
		}
	}

	fn filter(user_id: Uuid) -> RequestLogFilter {
		RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_and_provider_filters_apply_to_list_summary_and_export(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let mut first = success_record(user_id, None, None, "OpenAI/GPT-4o-mini");
		first.provider = Some("openai");
		RequestLog::record(&pool, first).await?;
		let mut second = success_record(user_id, None, None, "anthropic/claude-3-5-sonnet");
		second.provider = Some("anthropic");
		RequestLog::record(&pool, second).await?;
		RequestLog::record(&pool, success_record(user_id, None, None, "local/untagged")).await?;
		let by_model = RequestLogFilter {
			model: Some("gpt-4O".to_string()),
			..filter(user_id)
		};
		let by_provider = RequestLogFilter {
			provider: Some("ANTHROPIC".to_string()),
			..filter(user_id)
		};
		let by_both = RequestLogFilter {
			model: Some("claude".to_string()),
			provider: Some("openai".to_string()),
			..filter(user_id)
		};
		let by_partial_provider = RequestLogFilter {
			provider: Some("anthro".to_string()),
			..filter(user_id)
		};
		let listed = RequestLog::list(&pool, by_model.clone(), &paged(1, 50)).await?.items;
		assert_eq!(listed.len(), 1);
		assert_eq!(listed[0].public_model_id, "OpenAI/GPT-4o-mini");
		assert_eq!(RequestLog::list(&pool, by_provider.clone(), &paged(1, 50)).await?.items.len(), 1);
		assert!(RequestLog::list(&pool, by_both.clone(), &paged(1, 50)).await?.items.is_empty());
		assert!(RequestLog::list(&pool, by_partial_provider, &paged(1, 50)).await?.items.is_empty());
		assert_eq!(RequestLog::summary(&pool, by_model.clone()).await?.total_requests, 1);
		assert_eq!(RequestLog::summary(&pool, by_both).await?.total_requests, 0);
		assert_eq!(RequestLog::summary(&pool, filter(user_id)).await?.total_requests, 3);
		let exported = RequestLog::list_for_export(&pool, by_provider).await?;
		assert_eq!(exported.len(), 1);
		assert_eq!(exported[0].provider.as_deref(), Some("anthropic"));
		assert_eq!(RequestLog::list_for_export(&pool, by_model).await?.len(), 1);
		Ok(())
	}

	#[test]
	fn text_filters_are_trimmed_and_empty_ones_dropped() {
		assert_eq!(RequestLogFilter::text(Some("  openai ")), Some("openai".to_string()));
		assert_eq!(RequestLogFilter::text(Some("   ")), None);
		assert_eq!(RequestLogFilter::text(None), None);
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
		let Page { has_more, items } = RequestLog::list(&pool, filter(user_id), &paged(1, 50)).await?;
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
		assert_eq!(RequestLog::list(&pool, filter(user_id), &paged(1, 50)).await?.items.len(), 2);
		assert_eq!(RequestLog::list(&pool, by_team, &paged(1, 50)).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_key, &paged(1, 50)).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_status, &paged(1, 50)).await?.items.len(), 1);
		assert_eq!(RequestLog::list(&pool, by_success, &paged(1, 50)).await?.items.len(), 1);
		assert!(RequestLog::list(&pool, from_future, &paged(1, 50)).await?.items.is_empty());
		assert!(RequestLog::list(&pool, to_past, &paged(1, 50)).await?.items.is_empty());
		let _: DateTime<Utc> = Utc::now();
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_paginates_with_has_more(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		for _ in 0..3 {
			RequestLog::record(&pool, success_record(user_id, None, None, "openai/gpt-4o-mini")).await?;
		}
		let first = RequestLog::list(&pool, filter(user_id), &paged(1, 2)).await?;
		assert!(first.has_more);
		assert_eq!(first.items.len(), 2);
		let second = RequestLog::list(&pool, filter(user_id), &paged(2, 2)).await?;
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
	async fn finish_reason_and_first_token_time_are_stored_listed_and_exported(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let record = RequestLogRecord {
			finish_reason: Some("tool_calls"),
			time_to_first_token_ms: Some(250),
			..success_record(user_id, None, None, "openai/gpt-4o-mini")
		};
		let stored = RequestLog::record(&pool, record).await?;
		assert_eq!(stored.finish_reason.as_deref(), Some("tool_calls"));
		assert_eq!(stored.time_to_first_token_ms, Some(250));
		let listed = RequestLog::list(&pool, filter(user_id), &paged(1, 10)).await?;
		assert_eq!(listed.items[0].finish_reason.as_deref(), Some("tool_calls"));
		assert_eq!(listed.items[0].time_to_first_token_ms, Some(250));
		let exported = RequestLog::list_for_export(&pool, filter(user_id)).await?;
		assert_eq!(exported[0].finish_reason.as_deref(), Some("tool_calls"));
		assert_eq!(exported[0].time_to_first_token_ms, Some(250));
		let json = serde_json::to_value(&exported[0])?;
		assert_eq!(json["finish_reason"], "tool_calls");
		assert_eq!(json["time_to_first_token_ms"], 250);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unknown_finish_reasons_are_rejected_by_the_database(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let record = RequestLogRecord {
			finish_reason: Some("bogus"),
			..success_record(user_id, None, None, "openai/gpt-4o-mini")
		};
		assert!(RequestLog::record(&pool, record).await.is_err());
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
		assert!(RequestLog::list(&pool, RequestLogFilter::default(), &paged(1, 50)).await?.items.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod categories {
	use crate::tests::support::paged;
	use super::support::{TestResult, create_key, create_team, create_user};
	use crate::types::{RequestLog, RequestLogErrorCategory, RequestLogFailure, RequestLogFilter, RequestLogOwner, RequestLogRecord, RequestLogStatus, RequestLogTrace};
	use crate::types::RequestLogContext;
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	const ALL: [RequestLogErrorCategory; 7] = [
		RequestLogErrorCategory::Upstream,
		RequestLogErrorCategory::Interrupted,
		RequestLogErrorCategory::Budget,
		RequestLogErrorCategory::RateLimit,
		RequestLogErrorCategory::Auth,
		RequestLogErrorCategory::ModelAccess,
		RequestLogErrorCategory::Internal,
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
		let owner = RequestLogOwner { user_id: Some(user_id), team_id, api_key_id };
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
		assert_eq!(RequestLog::list(&pool, by_category.clone(), &paged(1, 50)).await?.items.len(), 1);
		by_category.error_category = Some(RequestLogErrorCategory::Budget);
		assert!(RequestLog::list(&pool, by_category, &paged(1, 50)).await?.items.is_empty());
		let exported = RequestLog::list_for_export(&pool, RequestLogFilter { user_id: Some(user_id), ..RequestLogFilter::default() }).await?;
		assert_eq!(exported[0].request_id, Some(context.request_id));
		assert_eq!(exported[0].error_category.as_deref(), Some("rate_limit"));
		assert_eq!(exported[0].finish_reason, None);
		assert_eq!(exported[0].time_to_first_token_ms, None);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_repeated_request_id_merges_into_one_row(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let owner = RequestLogOwner { user_id: Some(user_id), team_id, api_key_id };
		let context = RequestLogContext::start();
		let first = RequestLogRecord {
			user_id: Some(user_id),
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
			finish_reason: None,
			time_to_first_token_ms: None,
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
	async fn a_repeated_request_id_keeps_finish_reason_and_first_token_time(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let owner = RequestLogOwner { user_id: Some(user_id), team_id, api_key_id };
		let context = RequestLogContext::start();
		let record = RequestLogRecord {
			user_id: Some(user_id),
			team_id: Some(team_id),
			api_key_id: Some(api_key_id),
			model_id: None,
			public_model_id: "provider/model",
			request_type: "gateway.chat",
			status: RequestLogStatus::Success,
			status_code: None,
			input_tokens: 5,
			output_tokens: 5,
			reasoning_tokens: 0,
			cost_total: Decimal::ZERO,
			request_id: Some(context.request_id),
			provider: None,
			error_category: None,
			streaming: true,
			latency_ms: Some(300),
			finish_reason: Some("stop"),
			time_to_first_token_ms: Some(120),
		};
		RequestLog::record(&pool, record).await?;
		let merged = RequestLog::record_failure(
			&pool,
			&trace(context, owner),
			RequestLogFailure {
				category: RequestLogErrorCategory::Interrupted,
				status_code: None,
			},
		)
		.await?;
		assert_eq!(merged.finish_reason.as_deref(), Some("stop"));
		assert_eq!(merged.time_to_first_token_ms, Some(120));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rows_without_a_request_id_never_collide(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id).await?;
		let api_key_id = create_key(&pool, user_id, team_id).await?;
		let mut owner = RequestLogOwner { user_id: Some(user_id), team_id, api_key_id };
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
				finish_reason: None,
				time_to_first_token_ms: None,
			};
			RequestLog::record(&pool, record).await?;
		}
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gateway_request_logs").fetch_one(&pool).await?;
		assert_eq!(count, 2);
		owner.user_id = Some(Uuid::new_v4());
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
	use crate::tests::support::paged;
	use crate::tests::gateway::support::{TestResult, create_provider_model, create_team, create_user, insert_key, job_state};
	use crate::types::{REQUEST_ID_METADATA_KEY, RequestLog, RequestLogFilter, RequestLogResponse, STARTED_AT_METADATA_KEY, STREAMING_METADATA_KEY};
	use crate::utils::omniference_cost::OxideCostQueue;
	use omniference::catalog::UsageBreakdown;
	use omniference::middleware::cost::{CostFinalization, CostRecord, CostSink};
	use omniference::stream::{CostDetails, FinishReason};
	use std::time::Duration;
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
		Ok(RequestLog::list(pool, filter, &paged(1, 50)).await?.items)
	}

	async fn finalize(pool: &PgPool, setup: &Setup, finalization: CostFinalization) -> TestResult<Uuid> {
		let request_id = Uuid::new_v4();
		let queue = OxideCostQueue::spawn(job_state(pool.clone()));
		queue.record_with_context(&cost_record(setup, request_id, finalization));
		queue.shutdown().await;
		Ok(request_id)
	}

	fn cost_record(setup: &Setup, request_id: Uuid, finalization: CostFinalization) -> CostRecord {
		CostRecord {
			provider: "Final Provider".to_owned(),
			model: "final-model".to_owned(),
			cost: cost(),
			finalization,
			metadata: metadata(setup, request_id),
			usage: usage(),
			finish_reason: Some(FinishReason::Length),
			time_to_first_token: Some(Duration::from_millis(321)),
		}
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
			assert_eq!(logged[0].finish_reason.as_deref(), Some("length"));
			assert_eq!(logged[0].time_to_first_token_ms, Some(321));
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn non_streaming_requests_have_no_time_to_first_token(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let mut record = cost_record(&setup, Uuid::new_v4(), CostFinalization::Done);
		record.metadata.insert(STREAMING_METADATA_KEY.to_string(), "false".to_string());
		let queue = OxideCostQueue::spawn(job_state(pool.clone()));
		queue.record_with_context(&record);
		queue.shutdown().await;
		let logged = rows(&pool, setup.user_id).await?;
		assert_eq!(logged.len(), 1);
		assert!(!logged[0].streaming);
		assert_eq!(logged[0].finish_reason.as_deref(), Some("length"));
		assert_eq!(logged[0].time_to_first_token_ms, None);
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
				user_id: Some(setup.user_id),
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
	use crate::tests::support::paged;
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
		Ok(RequestLog::list(pool, filter, &paged(1, 50)).await?.items)
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

#[cfg(test)]
mod dropped_streams {
	use crate::tests::support::paged;
	use super::support::{TestResult, create_key, create_team, create_user};
	use crate::types::{
		RequestLog, RequestLogAbort, RequestLogContext, RequestLogErrorCategory, RequestLogFilter, RequestLogOwner, RequestLogRecord, RequestLogResponse, RequestLogStatus, RequestLogTrace,
		ReservedBody,
	};
	use ::axum::body::{Body, Bytes, HttpBody};
	use futures_util::{StreamExt, stream};
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use std::convert::Infallible;
	use std::pin::Pin;
	use std::time::Duration;
	use uuid::Uuid;

	struct Setup {
		user_id: Uuid,
		owner: RequestLogOwner,
		context: RequestLogContext,
	}

	async fn setup(pool: &PgPool) -> TestResult<Setup> {
		let user_id = create_user(pool).await?;
		let team_id = create_team(pool, user_id).await?;
		let api_key_id = create_key(pool, user_id, team_id).await?;
		Ok(Setup {
			user_id,
			owner: RequestLogOwner { user_id: Some(user_id), team_id, api_key_id },
			context: RequestLogContext::start(),
		})
	}

	fn abort_log(pool: &PgPool, setup: &Setup) -> RequestLogAbort {
		RequestLogAbort::new(
			pool,
			&RequestLogTrace {
				context: setup.context,
				owner: setup.owner,
				request_type: "gateway.chat",
				public_model_id: "provider/model",
				model_id: None,
				provider: Some("provider"),
				streaming: true,
			},
		)
	}

	fn endless_body() -> Body {
		let first = stream::once(async { Ok::<_, Infallible>(Bytes::from_static(b"data: hello\n\n")) });
		Body::from_stream(first.chain(stream::pending()))
	}

	fn finite_body() -> Body {
		Body::from_stream(stream::iter([Ok::<_, Infallible>(Bytes::from_static(b"data: hello\n\n"))]))
	}

	async fn next_frame<T: Send + Unpin + 'static>(body: &mut ReservedBody<T>) -> bool {
		futures_util::future::poll_fn(|context| Pin::new(&mut *body).poll_frame(context)).await.is_some()
	}

	async fn logged(pool: &PgPool, user_id: Uuid) -> TestResult<Vec<RequestLogResponse>> {
		let filter = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		Ok(RequestLog::list(pool, filter, &paged(1, 50)).await?.items)
	}

	async fn wait_for_rows(pool: &PgPool, user_id: Uuid) -> TestResult<Vec<RequestLogResponse>> {
		for _ in 0..100 {
			let rows = logged(pool, user_id).await?;
			if !rows.is_empty() {
				return Ok(rows);
			}
			tokio::time::sleep(Duration::from_millis(20)).await;
		}
		Ok(Vec::new())
	}

	fn usage_record(setup: &Setup, status: RequestLogStatus, category: Option<RequestLogErrorCategory>) -> RequestLogRecord<'static> {
		RequestLogRecord {
			user_id: Some(setup.user_id),
			team_id: Some(setup.owner.team_id),
			api_key_id: Some(setup.owner.api_key_id),
			model_id: None,
			public_model_id: "provider/model",
			request_type: "gateway.chat",
			status,
			status_code: None,
			input_tokens: 30,
			output_tokens: 12,
			reasoning_tokens: 0,
			cost_total: Decimal::new(2, 3),
			request_id: Some(setup.context.request_id),
			provider: Some("provider"),
			error_category: category,
			streaming: true,
			latency_ms: Some(80),
			finish_reason: None,
			time_to_first_token_ms: None,
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_stream_dropped_before_usage_is_logged_as_interrupted(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let mut body = ReservedBody::new(endless_body(), ()).with_abort_log(abort_log(&pool, &setup));
		assert!(next_frame(&mut body).await);
		drop(body);
		let rows = wait_for_rows(&pool, setup.user_id).await?;
		assert_eq!(rows.len(), 1);
		assert_eq!(rows[0].status, "error");
		assert_eq!(rows[0].error_category.as_deref(), Some("interrupted"));
		assert_eq!(rows[0].request_id, Some(setup.context.request_id));
		assert_eq!(rows[0].public_model_id, "provider/model");
		assert_eq!(rows[0].provider.as_deref(), Some("provider"));
		assert_eq!(rows[0].status_code, None);
		assert!(rows[0].streaming);
		assert_eq!(rows[0].input_tokens, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_stream_read_to_the_end_writes_nothing(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let mut body = ReservedBody::new(finite_body(), ()).with_abort_log(abort_log(&pool, &setup));
		assert!(next_frame(&mut body).await);
		assert!(!next_frame(&mut body).await);
		drop(body);
		tokio::time::sleep(Duration::from_millis(300)).await;
		assert!(logged(&pool, setup.user_id).await?.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_empty_body_dropped_unread_writes_nothing(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let body = ReservedBody::new(Body::empty(), ()).with_abort_log(abort_log(&pool, &setup));
		drop(body);
		tokio::time::sleep(Duration::from_millis(300)).await;
		assert!(logged(&pool, setup.user_id).await?.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_drop_after_usage_keeps_the_single_usage_row(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		RequestLog::record(&pool, usage_record(&setup, RequestLogStatus::Success, None)).await?;
		let mut body = ReservedBody::new(endless_body(), ()).with_abort_log(abort_log(&pool, &setup));
		assert!(next_frame(&mut body).await);
		drop(body);
		tokio::time::sleep(Duration::from_millis(300)).await;
		let rows = logged(&pool, setup.user_id).await?;
		assert_eq!(rows.len(), 1);
		assert_eq!(rows[0].status, "success");
		assert_eq!(rows[0].error_category, None);
		assert_eq!(rows[0].input_tokens, 30);
		assert_eq!(rows[0].cost_total, Decimal::new(2, 3));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn usage_arriving_after_the_drop_row_fills_it_in(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let mut body = ReservedBody::new(endless_body(), ()).with_abort_log(abort_log(&pool, &setup));
		assert!(next_frame(&mut body).await);
		drop(body);
		wait_for_rows(&pool, setup.user_id).await?;
		RequestLog::record(&pool, usage_record(&setup, RequestLogStatus::Error, Some(RequestLogErrorCategory::Interrupted))).await?;
		let interrupted = logged(&pool, setup.user_id).await?;
		assert_eq!(interrupted.len(), 1);
		assert_eq!(interrupted[0].status, "error");
		assert_eq!(interrupted[0].input_tokens, 30);
		RequestLog::record(&pool, usage_record(&setup, RequestLogStatus::Success, None)).await?;
		let finished = logged(&pool, setup.user_id).await?;
		assert_eq!(finished.len(), 1);
		assert_eq!(finished[0].status, "success");
		assert_eq!(finished[0].error_category, None);
		Ok(())
	}
}

#[cfg(test)]
mod unlogged_failures {
	use crate::tests::support::paged;
	use super::support::TestResult;
	use crate::tests::gateway::support::{create_provider_model, create_team, create_user, insert_key};
	use crate::types::{
		BudgetAdmission, GatewayAuthContext, GatewayCredential, GatewayInference, GatewayLimitError, GatewayModelAccessError, GatewayRequestScope, RequestLog, RequestLogContext,
		RequestLogFilter, RequestLogResponse,
	};
	use crate::routes::gateway::{reject_limited, reject_model_access};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn context(pool: &PgPool) -> TestResult<(Uuid, GatewayAuthContext)> {
		crate::config::Config::init(pool).await;
		crate::i18n::I18n::init(pool).await;
		let user_id = create_user(pool).await?;
		let team_id = create_team(pool, user_id, "Failure Team", true).await?;
		let (_, token) = insert_key(pool, Some(user_id), team_id, json!(["inference:write"])).await?;
		Ok((user_id, GatewayCredential::authenticate(pool, &token).await?))
	}

	async fn inference(pool: &PgPool) -> TestResult<GatewayInference> {
		let (_, model_id) = create_provider_model(pool, "Provider", "model").await?;
		Ok(GatewayInference {
			model_id,
			engine_model_id: "model".to_string(),
			public_model_id: "provider/model".to_string(),
			provider_name: "Provider".to_string(),
			max_output_tokens: None,
			context_length: None,
			budget: BudgetAdmission { reservation: None, warn: false },
		})
	}

	async fn only_row(pool: &PgPool, user_id: Uuid) -> TestResult<RequestLogResponse> {
		let filter = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		let mut rows = RequestLog::list(pool, filter, &paged(1, 50)).await?.items;
		assert_eq!(rows.len(), 1);
		Ok(rows.remove(0))
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_unavailable_is_logged_as_budget(pool: PgPool) -> TestResult {
		let (user_id, context) = context(&pool).await?;
		let log = RequestLogContext::start();
		let scope = GatewayRequestScope { db: &pool, context: &context, log, usage_type: "gateway.chat", streaming: false };
		let response = reject_model_access(&scope, None, GatewayModelAccessError::BudgetUnavailable, "provider/model").await;
		assert_eq!(response.status().as_u16(), 503);
		let row = only_row(&pool, user_id).await?;
		assert_eq!(row.error_category.as_deref(), Some("budget"));
		assert_eq!(row.status_code, Some(503));
		assert_eq!(row.request_id, Some(log.request_id));
		assert_eq!(row.public_model_id, "provider/model");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn database_errors_are_logged_as_internal(pool: PgPool) -> TestResult {
		let (user_id, context) = context(&pool).await?;
		let scope = GatewayRequestScope { db: &pool, context: &context, log: RequestLogContext::start(), usage_type: "gateway.responses", streaming: true };
		let response = reject_model_access(&scope, None, GatewayModelAccessError::Database(sqlx::Error::PoolTimedOut), "provider/model").await;
		assert_eq!(response.status().as_u16(), 500);
		let row = only_row(&pool, user_id).await?;
		assert_eq!(row.error_category.as_deref(), Some("internal"));
		assert_eq!(row.status, "error");
		assert_eq!(row.status_code, Some(500));
		assert_eq!(row.request_type, "gateway.responses");
		assert!(row.streaming);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn limiter_overload_is_logged_as_rate_limit(pool: PgPool) -> TestResult {
		let (user_id, context) = context(&pool).await?;
		let scope = GatewayRequestScope { db: &pool, context: &context, log: RequestLogContext::start(), usage_type: "gateway.chat", streaming: false };
		let inference = inference(&pool).await?;
		let response = reject_limited(&scope, &inference, GatewayLimitError::Overloaded).await;
		assert_eq!(response.status().as_u16(), 503);
		let row = only_row(&pool, user_id).await?;
		assert_eq!(row.error_category.as_deref(), Some("rate_limit"));
		assert_eq!(row.status_code, Some(503));
		assert_eq!(row.model_id, Some(inference.model_id));
		assert_eq!(row.provider.as_deref(), Some("Provider"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn limiter_unavailable_is_logged_as_internal(pool: PgPool) -> TestResult {
		let (user_id, context) = context(&pool).await?;
		let scope = GatewayRequestScope { db: &pool, context: &context, log: RequestLogContext::start(), usage_type: "gateway.chat", streaming: false };
		let inference = inference(&pool).await?;
		let response = reject_limited(&scope, &inference, GatewayLimitError::Unavailable).await;
		assert_eq!(response.status().as_u16(), 503);
		let row = only_row(&pool, user_id).await?;
		assert_eq!(row.error_category.as_deref(), Some("internal"));
		assert_eq!(row.status_code, Some(503));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_models_route_is_not_an_inference_request_and_stays_unlogged(pool: PgPool) -> TestResult {
		let (user_id, context) = context(&pool).await?;
		let state = crate::tests::gateway::support::job_state(pool.clone());
		let response = crate::routes::gateway::run_models(&state, &context).await;
		assert_eq!(response.status().as_u16(), 200);
		let filter = RequestLogFilter {
			user_id: Some(user_id),
			..RequestLogFilter::default()
		};
		assert!(RequestLog::list(&pool, filter, &paged(1, 50)).await?.items.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod service_scope {
	use crate::tests::support::paged;
	use super::support::{TestResult, create_team, create_user};
	use crate::types::{RequestLog, RequestLogQuery, RequestLogRecord, RequestLogScopeError, RequestLogStatus};
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn record(user_id: Option<Uuid>, team_id: Uuid, model: &str) -> RequestLogRecord<'_> {
		RequestLogRecord {
			user_id,
			team_id: Some(team_id),
			api_key_id: None,
			model_id: None,
			public_model_id: model,
			request_type: "gateway.chat",
			status: RequestLogStatus::Success,
			status_code: None,
			input_tokens: 1,
			output_tokens: 1,
			reasoning_tokens: 0,
			cost_total: Decimal::ZERO,
			request_id: None,
			provider: None,
			error_category: None,
			streaming: false,
			latency_ms: None,
			finish_reason: None,
			time_to_first_token_ms: None,
		}
	}

	fn query(scope: Option<&str>, team_id: Option<Uuid>) -> RequestLogQuery {
		RequestLogQuery {
			scope: scope.map(str::to_owned),
			team_id,
			api_key_id: None,
			status: None,
			error_category: None,
			model: None,
			provider: None,
			from: None,
			to: None,
		}
	}

	async fn make_admin(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> TestResult {
		sqlx::query("UPDATE team_members SET role = 'admin' WHERE team_id = $1 AND user_id = $2")
			.bind(team_id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(())
	}

	async fn add_member(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> TestResult {
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(team_id).bind(user_id).execute(pool).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_sees_only_service_rows_of_their_team(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let member_id = create_user(&pool).await?;
		let team_id = create_team(&pool, admin_id).await?;
		add_member(&pool, team_id, member_id).await?;
		make_admin(&pool, team_id, admin_id).await?;
		let other_team = create_team(&pool, member_id).await?;
		RequestLog::record(&pool, record(None, team_id, "service/model")).await?;
		RequestLog::record(&pool, record(Some(member_id), team_id, "member/model")).await?;
		RequestLog::record(&pool, record(Some(admin_id), team_id, "admin/model")).await?;
		RequestLog::record(&pool, record(None, other_team, "other/model")).await?;

		let filter = query(Some("team_service"), Some(team_id)).scoped_filter(&pool, &admin_id, false).await?;
		let items = RequestLog::list(&pool, filter.clone(), &paged(1, 50)).await?.items;
		assert_eq!(items.len(), 1);
		assert_eq!(items[0].public_model_id, "service/model");
		assert_eq!(RequestLog::list_for_export(&pool, filter.clone()).await?.len(), 1);
		assert_eq!(RequestLog::summary(&pool, filter).await?.total_requests, 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn member_and_outsider_are_forbidden(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let member_id = create_user(&pool).await?;
		let outsider_id = create_user(&pool).await?;
		let team_id = create_team(&pool, admin_id).await?;
		add_member(&pool, team_id, member_id).await?;
		make_admin(&pool, team_id, admin_id).await?;
		let scoped = query(Some("team_service"), Some(team_id));
		assert!(matches!(scoped.scoped_filter(&pool, &member_id, false).await, Err(RequestLogScopeError::AdminRequired)));
		assert!(matches!(scoped.scoped_filter(&pool, &outsider_id, false).await, Err(RequestLogScopeError::Forbidden)));
		assert!(scoped.scoped_filter(&pool, &outsider_id, true).await.is_ok());
		assert!(matches!(query(Some("team_service"), None).scoped_filter(&pool, &admin_id, false).await, Err(RequestLogScopeError::Forbidden)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn mine_scope_is_unchanged(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let member_id = create_user(&pool).await?;
		let team_id = create_team(&pool, admin_id).await?;
		add_member(&pool, team_id, member_id).await?;
		make_admin(&pool, team_id, admin_id).await?;
		RequestLog::record(&pool, record(None, team_id, "service/model")).await?;
		RequestLog::record(&pool, record(Some(member_id), team_id, "member/model")).await?;
		RequestLog::record(&pool, record(Some(admin_id), team_id, "admin/model")).await?;
		for scope in [None, Some("mine")] {
			let filter = query(scope, None).scoped_filter(&pool, &admin_id, false).await?;
			let items = RequestLog::list(&pool, filter, &paged(1, 50)).await?.items;
			assert_eq!(items.len(), 1);
			assert_eq!(items[0].public_model_id, "admin/model");
		}
		Ok(())
	}
}
