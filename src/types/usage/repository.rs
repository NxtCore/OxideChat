use super::{AnalyticsDayModelRow, AnalyticsRow, UsageEvent, UsageEventRecord};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ProviderSpendRow {
	provider_id: Uuid,
	spent_amount: Decimal,
}

impl UsageEvent {
	pub async fn current_month_spend_by_provider(pool: &PgPool, provider_ids: &[Uuid]) -> Result<HashMap<Uuid, Decimal>, sqlx::Error> {
		if provider_ids.is_empty() {
			return Ok(HashMap::new());
		}
		let rows = sqlx::query_as::<_, ProviderSpendRow>(
			r#"SELECT provider_id AS provider_id, COALESCE(SUM(cost_total), 0)::numeric AS spent_amount
			   FROM usage_events
			   WHERE provider_id = ANY($1)
			     AND created_at >= date_trunc('month', NOW() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
			     AND created_at < (date_trunc('month', NOW() AT TIME ZONE 'UTC') + INTERVAL '1 month') AT TIME ZONE 'UTC'
			   GROUP BY provider_id"#,
		)
		.bind(provider_ids)
		.fetch_all(pool)
		.await?;
		let mut result = HashMap::with_capacity(provider_ids.len());
		result.extend(provider_ids.iter().copied().map(|id| (id, Decimal::ZERO)));
		result.extend(rows.into_iter().map(|row| (row.provider_id, row.spent_amount)));
		Ok(result)
	}

	pub async fn record(pool: &PgPool, params: UsageEventRecord<'_>) -> Result<Self, sqlx::Error> {
		Self::insert(pool, params).await
	}

	async fn insert<'e, E>(executor: E, params: UsageEventRecord<'_>) -> Result<Self, sqlx::Error>
	where
		E: sqlx::Executor<'e, Database = Postgres>,
	{
		sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO usage_events (
				user_id, team_id, model_id, provider_id, request_type,
				input_tokens, output_tokens, reasoning_tokens, cost_total
			)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
			RETURNING id, user_id, team_id, model_id, provider_id, request_type,
			          input_tokens, output_tokens, reasoning_tokens, cost_total, created_at
			"#,
		)
		.bind(params.user_id)
		.bind(params.team_id)
		.bind(params.model_id)
		.bind(params.provider_id)
		.bind(params.request_type)
		.bind(params.input_tokens)
		.bind(params.output_tokens)
		.bind(params.reasoning_tokens)
		.bind(params.cost_total)
		.fetch_one(executor)
		.await
	}

	pub async fn analytics(pool: &PgPool, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>, group_by: &str) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		let end = to.unwrap_or_else(Utc::now);
		let start = from.unwrap_or(end - Duration::days(30));
		match group_by {
			"user" => Self::by_user(pool, start, end).await,
			"team" => Self::by_team(pool, start, end).await,
			"day" => Self::by_day(pool, start, end, None).await,
			_ => Self::by_model(pool, start, end, None).await,
		}
	}

	pub async fn analytics_for_user(
		pool: &PgPool,
		user_id: &Uuid,
		from: Option<DateTime<Utc>>,
		to: Option<DateTime<Utc>>,
		group_by: &str,
	) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		let end = to.unwrap_or_else(Utc::now);
		let start = from.unwrap_or(end - Duration::days(30));
		match group_by {
			"day" => Self::by_day(pool, start, end, Some(user_id)).await,
			_ => Self::by_model(pool, start, end, Some(user_id)).await,
		}
	}

	pub async fn day_model_analytics(
		pool: &PgPool,
		from: Option<DateTime<Utc>>,
		to: Option<DateTime<Utc>>,
		user_id: Option<&Uuid>,
	) -> Result<Vec<AnalyticsDayModelRow>, sqlx::Error> {
		let end = to.unwrap_or_else(Utc::now);
		let start = from.unwrap_or(end - Duration::days(30));
		Self::by_day_by_model(pool, start, end, user_id).await
	}

	async fn by_model(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>, user_id: Option<&Uuid>) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		sqlx::query_as::<_, AnalyticsRow>(
			r#"
			SELECT m.id, COALESCE(m.display_name, '[deleted]') AS label,
			       COALESCE(SUM(ue.input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(ue.output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(ue.reasoning_tokens), 0)::bigint AS reasoning_tokens,
			       COALESCE(SUM(ue.cost_total), 0)::numeric AS cost_total,
			       COUNT(*)::bigint AS request_count
			FROM usage_events ue
			LEFT JOIN models m ON m.id = ue.model_id
			WHERE ue.created_at >= $1 AND ue.created_at <= $2
			  AND ($3::uuid IS NULL OR ue.user_id = $3)
			GROUP BY m.id, m.display_name
			ORDER BY cost_total DESC
			"#,
		)
		.bind(start)
		.bind(end)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	async fn by_user(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		sqlx::query_as::<_, AnalyticsRow>(
			r#"
			SELECT u.id, COALESCE(u.username, u.email) AS label,
			       COALESCE(SUM(ue.input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(ue.output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(ue.reasoning_tokens), 0)::bigint AS reasoning_tokens,
			       COALESCE(SUM(ue.cost_total), 0)::numeric AS cost_total,
			       COUNT(*)::bigint AS request_count
			FROM usage_events ue
			JOIN users u ON u.id = ue.user_id
			WHERE ue.created_at >= $1 AND ue.created_at <= $2
			GROUP BY u.id, u.username, u.email
			ORDER BY cost_total DESC
			"#,
		)
		.bind(start)
		.bind(end)
		.fetch_all(pool)
		.await
	}

	async fn by_team(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		sqlx::query_as::<_, AnalyticsRow>(
			r#"
			SELECT t.id, COALESCE(t.name, '') AS label,
			       COALESCE(SUM(ue.input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(ue.output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(ue.reasoning_tokens), 0)::bigint AS reasoning_tokens,
			       COALESCE(SUM(ue.cost_total), 0)::numeric AS cost_total,
			       COUNT(*)::bigint AS request_count
			FROM usage_events ue
			LEFT JOIN teams t ON t.id = ue.team_id
			WHERE ue.created_at >= $1 AND ue.created_at <= $2
			GROUP BY t.id, t.name
			ORDER BY cost_total DESC
			"#,
		)
		.bind(start)
		.bind(end)
		.fetch_all(pool)
		.await
	}

	async fn by_day(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>, user_id: Option<&Uuid>) -> Result<Vec<AnalyticsRow>, sqlx::Error> {
		sqlx::query_as::<_, AnalyticsRow>(
			r#"
			SELECT NULL::uuid AS id, to_char(date_trunc('day', created_at), 'YYYY-MM-DD') AS label,
			       COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(reasoning_tokens), 0)::bigint AS reasoning_tokens,
			       COALESCE(SUM(cost_total), 0)::numeric AS cost_total,
			       COUNT(*)::bigint AS request_count
			FROM usage_events
			WHERE created_at >= $1 AND created_at <= $2
			  AND ($3::uuid IS NULL OR user_id = $3)
			GROUP BY date_trunc('day', created_at)
			ORDER BY label ASC
			"#,
		)
		.bind(start)
		.bind(end)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	async fn by_day_by_model(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>, user_id: Option<&Uuid>) -> Result<Vec<AnalyticsDayModelRow>, sqlx::Error> {
		sqlx::query_as::<_, AnalyticsDayModelRow>(
			r#"
			SELECT to_char(date_trunc('day', ue.created_at), 'YYYY-MM-DD') AS day,
			       m.id AS model_id, COALESCE(m.display_name, '[deleted]') AS model_name,
			       COALESCE(SUM(ue.input_tokens), 0)::bigint AS input_tokens,
			       COALESCE(SUM(ue.output_tokens), 0)::bigint AS output_tokens,
			       COALESCE(SUM(ue.reasoning_tokens), 0)::bigint AS reasoning_tokens,
			       COALESCE(SUM(ue.cost_total), 0)::numeric AS cost_total,
			       COUNT(*)::bigint AS request_count
			FROM usage_events ue
			LEFT JOIN models m ON m.id = ue.model_id
			WHERE ue.created_at >= $1 AND ue.created_at <= $2
			  AND ($3::uuid IS NULL OR ue.user_id = $3)
			GROUP BY date_trunc('day', ue.created_at), m.id, m.display_name
			ORDER BY day ASC, cost_total DESC
			"#,
		)
		.bind(start)
		.bind(end)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}
}
