use std::collections::HashMap;
use chrono::{Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use super::{Provider, ProviderBillingConnection, ProviderBillingMetric, ProviderBillingMetricKind, ProviderBillingMetricResponse, ProviderBillingOverviewResponse, ProviderBillingOverviewRow, ProviderBillingStatus, ProviderKind, ProviderLocalSpendResponse};
use uuid::Uuid;
use crate::types::UsageEvent;
use crate::utils::encryption::encrypt_object_values;

fn should_show_billing_overview(status: &ProviderBillingStatus) -> bool {
	!matches!(status, ProviderBillingStatus::Unsupported)
}

impl Provider {
	pub async fn list_for_admin(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE owner_id IS NULL
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn list_enabled_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE owner_id IS NULL AND is_enabled = true
			ORDER BY name
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn find_for_admin(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			FROM providers
			WHERE id = $1 AND owner_id IS NULL
			"#,
		)
		.bind(id)
		.fetch_optional(pool)
		.await
	}

	pub async fn create_system(
		pool: &sqlx::PgPool,
		kind: &super::ProviderKind,
		name: &str,
		base_url: &str,
		api_key: Option<&str>,
		extra_headers: &serde_json::Value,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		let protected_headers = encrypt_object_values(extra_headers, None).map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
		sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO providers (owner_id, kind, name, base_url, api_key, extra_headers, is_enabled)
			VALUES (NULL, $1, $2, $3, $4, $5, $6)
			RETURNING id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled, created_at, updated_at
			"#,
		)
		.bind(kind)
		.bind(name)
		.bind(base_url)
		.bind(api_key)
		.bind(&protected_headers)
		.bind(is_enabled)
		.fetch_one(pool)
		.await
	}

	pub async fn delete_system(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query(
			r#"
			DELETE FROM providers
			WHERE id = $1 AND owner_id IS NULL
			"#,
		)
		.bind(id)
		.execute(pool)
		.await?;

		Ok(result.rows_affected() > 0)
	}
}

// Budgeting
impl Provider {
	async fn billing_rows(pool: &PgPool, provider_id: Option<&Uuid>) -> Result<Vec<ProviderBillingOverviewRow>, sqlx::Error> {
		sqlx::query_as::<_, ProviderBillingOverviewRow>(
			r#"SELECT p.id AS provider_id, p.kind AS provider_kind,
				          c.credential, (p.api_key IS NOT NULL) AS has_provider_api_key,
			          c.last_status, c.last_error_code, c.last_synced_at,
			          s.metric_kind, s.currency, s.period_start, s.period_end, s.limit_amount,
			          s.spent_amount, s.remaining_amount, s.is_hard_limit, s.thresholds, s.details, s.fetched_at
		   FROM providers p
		   LEFT JOIN provider_billing_connections c ON c.provider_id=p.id
		   LEFT JOIN provider_billing_snapshots s ON s.provider_id=p.id
		   WHERE p.owner_id IS NULL AND ($1::uuid IS NULL OR p.id=$1)
		   ORDER BY p.name"#,
		)
			.bind(provider_id)
			.fetch_all(pool)
			.await
	}

	fn overview_from_row(row: ProviderBillingOverviewRow, spend: &HashMap<Uuid, Decimal>) -> ProviderBillingOverviewResponse {
		let now = Utc::now();
		let supports_upstream_billing = row.provider_kind == ProviderKind::Openrouter;
		let start = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).single().unwrap_or(now);
		let end = if now.month() == 12 {
			Utc.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0).single().unwrap_or(now)
		} else {
			Utc.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0).single().unwrap_or(now)
		};
		let status = if supports_upstream_billing {
			row.last_status
				.as_deref()
				.and_then(ProviderBillingStatus::from_str)
				.unwrap_or(ProviderBillingStatus::NotConfigured)
		} else {
			ProviderBillingStatus::Unsupported
		};
		let refresh_failed = matches!(status, ProviderBillingStatus::Unauthorized | ProviderBillingStatus::UpstreamError);
		let is_stale = supports_upstream_billing
			&& (row.fetched_at.is_some_and(|fetched| now.signed_duration_since(fetched).num_minutes() > 30) || refresh_failed && row.fetched_at.is_some());
		let upstream = supports_upstream_billing.then(|| {
			row.metric_kind.as_deref().and_then(ProviderBillingMetricKind::from_str).map(|metric_kind| ProviderBillingMetricResponse {
				metric_kind,
				currency: row.currency.unwrap_or_else(|| "USD".to_string()),
				period_start: row.period_start,
				period_end: row.period_end,
				limit_amount: row.limit_amount,
				spent_amount: row.spent_amount,
				remaining_amount: row.remaining_amount,
				is_hard_limit: row.is_hard_limit.unwrap_or(false),
				thresholds: row.thresholds.and_then(|value| serde_json::from_value(value).ok()).unwrap_or_default(),
			})
		}).flatten();
		let is_enabled = match row.provider_kind {
			ProviderKind::Openrouter => row.credential.is_some() || row.has_provider_api_key,
			ProviderKind::Openai | ProviderKind::Anthropic | ProviderKind::Google | ProviderKind::OpenaiCompat | ProviderKind::Custom => false,
		};
		ProviderBillingOverviewResponse {
			provider_id: row.provider_id,
			provider_kind: row.provider_kind,
			status,
			is_enabled,
			has_billing_credential: row.credential.is_some(),
			upstream,
			local: ProviderLocalSpendResponse {
				currency: "USD".to_string(),
				period_start: start,
				period_end: end,
				spent_amount: spend.get(&row.provider_id).copied().unwrap_or(Decimal::ZERO),
			},
			last_synced_at: row.last_synced_at,
			is_stale,
			error_code: row.last_error_code,
		}
	}


	pub async fn list_billing_overviews_for_admin(pool: &PgPool) -> Result<Vec<ProviderBillingOverviewResponse>, sqlx::Error> {
		let rows = Provider::billing_rows(pool, None).await?;
		let provider_ids: Vec<Uuid> = rows.iter().map(|row| row.provider_id).collect();
		let spend = UsageEvent::current_month_spend_by_provider(pool, &provider_ids).await?;
		Ok(rows
			.into_iter()
			.map(|row| Provider::overview_from_row(row, &spend))
			.filter(|overview| should_show_billing_overview(&overview.status))
			.collect())
	}

	pub async fn billing_overview_for_admin(pool: &PgPool, provider_id: &Uuid) -> Result<Option<ProviderBillingOverviewResponse>, sqlx::Error> {
		let mut rows = Provider::billing_rows(pool, Some(provider_id)).await?;
		let Some(row) = rows.pop() else { return Ok(None) };
		let spend = UsageEvent::current_month_spend_by_provider(pool, &[row.provider_id]).await?;
		let overview = Provider::overview_from_row(row, &spend);
		Ok(should_show_billing_overview(&overview.status).then_some(overview))
	}

	pub async fn find_billing_connection_for_admin(pool: &PgPool, provider_id: &Uuid) -> Result<Option<ProviderBillingConnection>, sqlx::Error> {
		sqlx::query_as::<_, ProviderBillingConnection>(
			r#"SELECT c.provider_id, c.credential, c.last_status, c.last_error_code, c.last_synced_at
			   FROM provider_billing_connections c
			   JOIN providers p ON p.id = c.provider_id
			   WHERE c.provider_id = $1 AND p.owner_id IS NULL"#,
		)
			.bind(provider_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn upsert_billing_connection_for_admin(
		pool: &PgPool,
		provider_id: &Uuid,
		credential: Option<&str>,
	) -> Result<Option<ProviderBillingConnection>, sqlx::Error> {
		sqlx::query_as::<_, ProviderBillingConnection>(
			r#"INSERT INTO provider_billing_connections (provider_id, credential, last_status)
			   SELECT id, $2, 'NOT_CONFIGURED'
			   FROM providers WHERE id = $1 AND owner_id IS NULL
			   ON CONFLICT (provider_id) DO UPDATE SET
			       credential = COALESCE(EXCLUDED.credential, provider_billing_connections.credential),
			       updated_at = NOW()
			   RETURNING provider_id, credential, last_status, last_error_code, last_synced_at"#,
		)
			.bind(provider_id)
			.bind(credential)
			.fetch_optional(pool)
			.await
	}

	pub async fn delete_billing_connection_for_admin(pool: &PgPool, provider_id: &Uuid) -> Result<bool, sqlx::Error> {
		let mut transaction = pool.begin().await?;
		sqlx::query("DELETE FROM provider_billing_snapshots WHERE provider_id = $1")
			.bind(provider_id)
			.execute(&mut *transaction)
			.await?;
		let result = sqlx::query("DELETE FROM provider_billing_connections c USING providers p WHERE c.provider_id = $1 AND p.id = c.provider_id AND p.owner_id IS NULL")
			.bind(provider_id)
			.execute(&mut *transaction)
			.await?;
		transaction.commit().await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn save_billing_snapshot(pool: &PgPool, provider_id: &Uuid, metric: &ProviderBillingMetric) -> Result<(), sqlx::Error> {
		let thresholds = serde_json::to_value(&metric.thresholds).unwrap_or_else(|_| serde_json::json!([]));
		let mut transaction = pool.begin().await?;
		sqlx::query(
			r#"INSERT INTO provider_billing_snapshots (
			       provider_id, metric_kind, currency, period_start, period_end, limit_amount,
			       spent_amount, remaining_amount, is_hard_limit, thresholds, details, fetched_at
			   ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
			   ON CONFLICT (provider_id) DO UPDATE SET
			       metric_kind=EXCLUDED.metric_kind, currency=EXCLUDED.currency,
			       period_start=EXCLUDED.period_start, period_end=EXCLUDED.period_end,
			       limit_amount=EXCLUDED.limit_amount, spent_amount=EXCLUDED.spent_amount,
			       remaining_amount=EXCLUDED.remaining_amount, is_hard_limit=EXCLUDED.is_hard_limit,
			       thresholds=EXCLUDED.thresholds, details=EXCLUDED.details, fetched_at=EXCLUDED.fetched_at"#,
		)
			.bind(provider_id)
			.bind(metric.metric_kind.as_str())
			.bind(&metric.currency)
			.bind(metric.period_start)
			.bind(metric.period_end)
			.bind(metric.limit_amount)
			.bind(metric.spent_amount)
			.bind(metric.remaining_amount)
			.bind(metric.is_hard_limit)
			.bind(thresholds)
			.bind(&metric.details)
			.bind(metric.fetched_at)
			.execute(&mut *transaction)
			.await?;
		sqlx::query("UPDATE provider_billing_connections SET last_status='AVAILABLE', last_error_code=NULL, last_synced_at=$2, updated_at=NOW() WHERE provider_id=$1")
			.bind(provider_id)
			.bind(metric.fetched_at)
			.execute(&mut *transaction)
			.await?;
		transaction.commit().await
	}

	pub async fn mark_billing_refresh_failure(pool: &PgPool, provider_id: &Uuid, status: ProviderBillingStatus, error_code: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE provider_billing_connections SET last_status=$2, last_error_code=$3, updated_at=NOW() WHERE provider_id=$1")
			.bind(provider_id)
			.bind(status.as_str())
			.bind(error_code)
			.execute(pool)
			.await?;
		Ok(())
	}

	pub async fn list_enabled_billing_connections(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"SELECT p.id, p.owner_id, p.kind, p.name, p.base_url, p.api_key, p.extra_headers, p.is_enabled, p.created_at, p.updated_at
			   FROM providers p
			   LEFT JOIN provider_billing_connections c ON c.provider_id=p.id
			   WHERE p.owner_id IS NULL
			     AND p.is_enabled=true
			     AND p.kind='OPENROUTER'
			     AND (c.credential IS NOT NULL OR p.api_key IS NOT NULL)
			   ORDER BY p.name"#,
		)
			.fetch_all(pool)
			.await
	}
}
