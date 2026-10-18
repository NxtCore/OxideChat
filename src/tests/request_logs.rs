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
		}
	}

	fn filter(user_id: Uuid) -> RequestLogFilter {
		RequestLogFilter {
			user_id: Some(user_id),
			team_id: None,
			api_key_id: None,
			status: None,
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
		sqlx::query("UPDATE request_logs SET created_at = NOW() - INTERVAL '10 days' WHERE user_id = $1")
			.bind(user_id)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE request_logs SET created_at = NOW() - INTERVAL '40 days' WHERE user_id = $1")
			.bind(other_id)
			.execute(&pool)
			.await?;
		assert_eq!(RequestLog::prune(&pool).await?, 2);
		assert!(RequestLog::list(&pool, RequestLogFilter::default(), 1, 50).await?.items.is_empty());
		Ok(())
	}
}
