use super::{
	REQUEST_LOG_SCOPE_TEAM_SERVICE, RequestLogQuery, RequestLogScopeError,
	DEFAULT_REQUEST_LOG_RETENTION_DAYS, REQUEST_LOG_EXPORT_LIMIT, RequestLog, RequestLogAbort, RequestLogErrorCategory, RequestLogFailure, RequestLogFilter, RequestLogRecord, RequestLogResponse, RequestLogStatus,
	RequestLogSummary, RequestLogTrace,
};
use crate::types::{BaseType, Team, axum::PaginatedResponse};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

impl BaseType for RequestLog {}

impl RequestLog {
	/// Inserts one request log row.
	///
	/// A second row for the same request id is merged into the first, so a
	/// request never shows up twice.
	///
	/// # Errors
	///
	/// Returns an error when the row cannot be stored.
	pub async fn record(pool: &PgPool, params: RequestLogRecord<'_>) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO gateway_request_logs (
				user_id, team_id, api_key_id, model_id, public_model_id, request_type, status, status_code,
				input_tokens, output_tokens, reasoning_tokens, cost_total, request_id, provider, error_category,
				streaming, latency_ms
			)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
			ON CONFLICT (request_id) WHERE request_id IS NOT NULL DO UPDATE SET
				status = CASE
					WHEN gateway_request_logs.error_category = 'interrupted' AND EXCLUDED.status = 'success' THEN 'success'
					ELSE gateway_request_logs.status
				END,
				status_code = COALESCE(gateway_request_logs.status_code, EXCLUDED.status_code),
				error_category = CASE
					WHEN gateway_request_logs.status = 'success' THEN NULL
					WHEN gateway_request_logs.error_category = 'interrupted' AND EXCLUDED.status = 'success' THEN NULL
					ELSE COALESCE(gateway_request_logs.error_category, EXCLUDED.error_category)
				END,
				provider = COALESCE(gateway_request_logs.provider, EXCLUDED.provider),
				input_tokens = GREATEST(gateway_request_logs.input_tokens, EXCLUDED.input_tokens),
				output_tokens = GREATEST(gateway_request_logs.output_tokens, EXCLUDED.output_tokens),
				reasoning_tokens = GREATEST(gateway_request_logs.reasoning_tokens, EXCLUDED.reasoning_tokens),
				cost_total = GREATEST(gateway_request_logs.cost_total, EXCLUDED.cost_total),
				latency_ms = GREATEST(gateway_request_logs.latency_ms, EXCLUDED.latency_ms)
			RETURNING id, user_id, team_id, api_key_id, model_id, public_model_id, request_type, status, status_code,
			          input_tokens, output_tokens, reasoning_tokens, cost_total, created_at, request_id, provider,
			          error_category, streaming, latency_ms
			"#,
		)
		.bind(params.user_id)
		.bind(params.team_id)
		.bind(params.api_key_id)
		.bind(params.model_id)
		.bind(params.public_model_id)
		.bind(params.request_type)
		.bind(params.status.as_str())
		.bind(params.status_code)
		.bind(params.input_tokens)
		.bind(params.output_tokens)
		.bind(params.reasoning_tokens)
		.bind(params.cost_total)
		.bind(params.request_id)
		.bind(params.provider)
		.bind(params.error_category.map(RequestLogErrorCategory::as_str))
		.bind(params.streaming)
		.bind(params.latency_ms)
		.fetch_one(pool)
		.await
	}

	/// Inserts an error row for a request that never produced a completion.
	///
	/// # Errors
	///
	/// Returns an error when the row cannot be stored.
	pub async fn record_failure(pool: &PgPool, trace: &RequestLogTrace<'_>, failure: RequestLogFailure) -> Result<Self, sqlx::Error> {
		Self::record(
			pool,
			RequestLogRecord {
				user_id: trace.owner.user_id,
				team_id: Some(trace.owner.team_id),
				api_key_id: Some(trace.owner.api_key_id),
				model_id: trace.model_id,
				public_model_id: trace.public_model_id,
				request_type: trace.request_type,
				status: RequestLogStatus::Error,
				status_code: failure.status_code,
				input_tokens: 0,
				output_tokens: 0,
				reasoning_tokens: 0,
				cost_total: Decimal::ZERO,
				request_id: Some(trace.context.request_id),
				provider: trace.provider,
				error_category: Some(failure.category),
				streaming: trace.streaming,
				latency_ms: Some(trace.context.latency_ms(Utc::now())),
			},
		)
		.await
	}

	/// Lists the caller's request logs, newest first, one page at a time.
	///
	/// # Errors
	///
	/// Returns an error when the rows cannot be read.
	pub async fn list(pool: &PgPool, filter: RequestLogFilter, page: i64, size: i64) -> Result<PaginatedResponse<RequestLogResponse>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let rows = Self::bind_filter(sqlx::query_as::<_, Self>(Self::SELECT), &filter)
			.bind(pagination.limit)
			.bind(pagination.offset)
			.fetch_all(pool)
			.await?;
		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(|row| row.to_response()).collect();
		Ok(PaginatedResponse { has_more, items })
	}

	/// Lists every request log matching the filter, bounded by the export limit.
	///
	/// # Errors
	///
	/// Returns an error when the rows cannot be read.
	pub async fn list_for_export(pool: &PgPool, filter: RequestLogFilter) -> Result<Vec<RequestLogResponse>, sqlx::Error> {
		let rows = Self::bind_filter(sqlx::query_as::<_, Self>(Self::SELECT), &filter)
			.bind(REQUEST_LOG_EXPORT_LIMIT)
			.bind(0_i64)
			.fetch_all(pool)
			.await?;
		Ok(rows.into_iter().map(|row| row.to_response()).collect())
	}

	/// Summarizes the logged requests matching the filter.
	///
	/// # Errors
	///
	/// Returns an error when the rows cannot be read.
	pub async fn summary(pool: &PgPool, filter: RequestLogFilter) -> Result<RequestLogSummary, sqlx::Error> {
		Self::bind_filter(
			sqlx::query_as::<_, RequestLogSummary>(
			r#"
			SELECT COUNT(*) AS total_requests,
			       COUNT(*) FILTER (WHERE status = 'error') AS error_requests,
			       COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '24 hours') AS requests_24h,
			       COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(cost_total), 0) AS cost_total,
			       MAX(created_at) AS last_request_at
			FROM gateway_request_logs
			WHERE ($1::uuid IS NULL OR user_id = $1)
			  AND ($2::uuid IS NULL OR team_id = $2)
			  AND ($3::uuid IS NULL OR api_key_id = $3)
			  AND ($4::text IS NULL OR status = $4)
			  AND ($5::text IS NULL OR error_category = $5)
			  AND ($6::text IS NULL OR POSITION(LOWER($6) IN LOWER(public_model_id)) > 0)
			  AND ($7::text IS NULL OR LOWER(provider) = LOWER($7))
			  AND ($8::timestamptz IS NULL OR created_at >= $8)
			  AND ($9::timestamptz IS NULL OR created_at <= $9)
			  AND (NOT $10::bool OR user_id IS NULL)
			"#,
			),
			&filter,
		)
		.fetch_one(pool)
		.await
	}

	/// Returns the user's retention in days, defaulting when unset.
	///
	/// # Errors
	///
	/// Returns an error when the setting cannot be read.
	pub async fn retention_days(pool: &PgPool, user_id: &Uuid) -> Result<i32, sqlx::Error> {
		let days: Option<i32> = sqlx::query_scalar("SELECT retention_days FROM request_log_retention WHERE user_id = $1")
			.bind(user_id)
			.fetch_optional(pool)
			.await?;
		Ok(days.unwrap_or(DEFAULT_REQUEST_LOG_RETENTION_DAYS))
	}

	/// Stores the user's retention in days.
	///
	/// # Errors
	///
	/// Returns an error when the setting cannot be stored.
	pub async fn set_retention_days(pool: &PgPool, user_id: &Uuid, retention_days: i32) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			INSERT INTO request_log_retention (user_id, retention_days)
			VALUES ($1, $2)
			ON CONFLICT (user_id) DO UPDATE SET retention_days = $2, updated_at = NOW()
			"#,
		)
		.bind(user_id)
		.bind(retention_days)
		.execute(pool)
		.await?;
		Ok(())
	}

	/// Deletes request logs past each user's retention.
	///
	/// # Errors
	///
	/// Returns an error when the rows cannot be deleted.
	pub async fn prune(pool: &PgPool) -> Result<u64, sqlx::Error> {
		let result = sqlx::query(
			r#"
			DELETE FROM gateway_request_logs r
			WHERE r.created_at < NOW() - COALESCE(
				(SELECT make_interval(days => rr.retention_days) FROM request_log_retention rr WHERE rr.user_id = r.user_id),
				make_interval(days => $1)
			)
			"#,
		)
		.bind(DEFAULT_REQUEST_LOG_RETENTION_DAYS)
		.execute(pool)
		.await?;
		Ok(result.rows_affected())
	}
}

impl RequestLogAbort {
	#[must_use]
	/// Captures what the log row of an interrupted response needs.
	pub fn new(pool: &PgPool, trace: &RequestLogTrace<'_>) -> Self {
		Self {
			pool: pool.clone(),
			context: trace.context,
			owner: trace.owner,
			request_type: trace.request_type.to_owned(),
			public_model_id: trace.public_model_id.to_owned(),
			model_id: trace.model_id,
			provider: trace.provider.map(str::to_owned),
			streaming: trace.streaming,
		}
	}

	/// Stores the interrupted request in the background; does nothing outside a Tokio runtime.
	///
	/// A request that already reported usage keeps its row, because rows merge by request id.
	pub fn record_in_background(self) {
		let Ok(runtime) = tokio::runtime::Handle::try_current() else {
			return;
		};
		runtime.spawn(async move {
			let trace = RequestLogTrace {
				context: self.context,
				owner: self.owner,
				request_type: &self.request_type,
				public_model_id: &self.public_model_id,
				model_id: self.model_id,
				provider: self.provider.as_deref(),
				streaming: self.streaming,
			};
			let failure = RequestLogFailure {
				category: RequestLogErrorCategory::Interrupted,
				status_code: None,
			};
			if let Err(error) = RequestLog::record_failure(&self.pool, &trace, failure).await {
				tracing::warn!(%error, "failed to record interrupted gateway request");
			}
		});
	}
}

impl RequestLogQuery {
	/// Resolves the filter for the caller, checking team administration for the service scope.
	///
	/// # Errors
	///
	/// Returns `RequestLogScopeError::Forbidden` when the caller is not a member of the requested team,
	/// `RequestLogScopeError::AdminRequired` when the caller is a member but not a team admin,
	/// and `RequestLogScopeError::Database` when membership cannot be checked.
	pub async fn scoped_filter(&self, pool: &PgPool, user_id: &Uuid, installation_admin: bool) -> Result<RequestLogFilter, RequestLogScopeError> {
		if self.scope.as_deref() != Some(REQUEST_LOG_SCOPE_TEAM_SERVICE) {
			return Ok(self.filter(*user_id));
		}
		let team_id = self.team_id.ok_or(RequestLogScopeError::Forbidden)?;
		if !installation_admin && !Team::is_admin(pool, &team_id, user_id).await? {
			return Err(if Team::is_member(pool, &team_id, user_id).await? {
				RequestLogScopeError::AdminRequired
			} else {
				RequestLogScopeError::Forbidden
			});
		}
		let mut filter = self.filter(*user_id);
		filter.user_id = None;
		filter.service_accounts_only = true;
		Ok(filter)
	}
}

impl RequestLog {
	const SELECT: &str = r#"
		SELECT id, user_id, team_id, api_key_id, model_id, public_model_id, request_type, status, status_code,
		       input_tokens, output_tokens, reasoning_tokens, cost_total, created_at, request_id, provider,
		       error_category, streaming, latency_ms
		FROM gateway_request_logs
		WHERE ($1::uuid IS NULL OR user_id = $1)
		  AND ($2::uuid IS NULL OR team_id = $2)
		  AND ($3::uuid IS NULL OR api_key_id = $3)
		  AND ($4::text IS NULL OR status = $4)
		  AND ($5::text IS NULL OR error_category = $5)
		  AND ($6::text IS NULL OR POSITION(LOWER($6) IN LOWER(public_model_id)) > 0)
		  AND ($7::text IS NULL OR LOWER(provider) = LOWER($7))
		  AND ($8::timestamptz IS NULL OR created_at >= $8)
		  AND ($9::timestamptz IS NULL OR created_at <= $9)
		  AND (NOT $10::bool OR user_id IS NULL)
		ORDER BY created_at DESC
		LIMIT $11 OFFSET $12
		"#;

	fn bind_filter<'q, O>(
		query: sqlx::query::QueryAs<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments>,
		filter: &'q RequestLogFilter,
	) -> sqlx::query::QueryAs<'q, sqlx::Postgres, O, sqlx::postgres::PgArguments> {
		query
			.bind(filter.user_id)
			.bind(filter.team_id)
			.bind(filter.api_key_id)
			.bind(filter.status.map(RequestLogStatus::as_str))
			.bind(filter.error_category.map(RequestLogErrorCategory::as_str))
			.bind(filter.model.as_deref())
			.bind(filter.provider.as_deref())
			.bind(filter.from)
			.bind(filter.to)
			.bind(filter.service_accounts_only)
	}
}
