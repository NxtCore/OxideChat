use super::{
	DEFAULT_BUDGET_THRESHOLD_PERCENT, DEFAULT_EVENT_WINDOW_DAYS, EVENT_KEY_ADMIN_DISABLED, EVENT_KEY_ADMIN_ENABLED, EVENT_KEY_ADMIN_REVOKED, EVENT_TEAM_ACCESS_UPDATED,
	EVENT_TEAM_BUDGET_UPDATED, EVENT_TEAM_LIMITS_UPDATED, EVENT_TEAM_MODELS_UPDATED, GatewayAdminError, GatewayAuditEntryResponse, GatewayAuditQuery, GatewayEventQuery, GatewayEventResponse,
	MAX_EVENT_BUDGET_KEYS, MAX_EVENT_WINDOW_DAYS, redact_metadata,
};
use crate::types::{Page, PageQuery};
use crate::types::{BaseType, Budget, BudgetSubject};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

const EVENT_KEY_EXPIRING: &str = "key_expiring";
const EVENT_BUDGET_THRESHOLD: &str = "budget_threshold";
const MAX_EXPIRING_KEYS: i64 = 1000;

const AUDIT_CATEGORIES: [(&str, &[&str]); 5] = [
	(
		"key",
		&[
			"gateway_api_key_created",
			"gateway_api_key_updated",
			"gateway_api_key_revoked",
			"gateway_api_key_rotated",
			EVENT_KEY_ADMIN_REVOKED,
			EVENT_KEY_ADMIN_DISABLED,
			EVENT_KEY_ADMIN_ENABLED,
		],
	),
	("limit", &["gateway_api_key_limits_updated", EVENT_TEAM_LIMITS_UPDATED]),
	("budget", &["gateway_api_key_budget_updated", "gateway_api_key_budget_cleared", EVENT_TEAM_BUDGET_UPDATED]),
	("model_policy", &[EVENT_TEAM_MODELS_UPDATED]),
	("access", &[EVENT_TEAM_ACCESS_UPDATED]),
];

#[derive(sqlx::FromRow)]
struct AuditRow {
	id: Uuid,
	event: String,
	actor_id: Option<Uuid>,
	actor_username: Option<String>,
	target_type: Option<String>,
	target_id: Option<Uuid>,
	target_label: Option<String>,
	metadata: Option<Value>,
	created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ExpiringKeyRow {
	id: Uuid,
	name: String,
	team_id: Uuid,
	expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct BudgetKeyRow {
	id: Uuid,
	name: String,
	team_id: Uuid,
}

impl BaseType for GatewayAuditEntryResponse {}

fn category_of(event: &str) -> &'static str {
	AUDIT_CATEGORIES
		.iter()
		.find(|(_, events)| events.contains(&event))
		.map_or("other", |(category, _)| category)
}

fn audited_events(query: &GatewayAuditQuery) -> Result<Vec<String>, GatewayAdminError> {
	let mut events = match query.category.as_deref().filter(|category| !category.is_empty()) {
		Some(category) => AUDIT_CATEGORIES
			.iter()
			.find(|(name, _)| *name == category)
			.map(|(_, events)| events.to_vec())
			.ok_or(GatewayAdminError::InvalidQuery)?,
		None => AUDIT_CATEGORIES.iter().flat_map(|(_, events)| events.iter().copied()).collect(),
	};
	if let Some(event) = query.event.as_deref().filter(|event| !event.is_empty()) {
		events.retain(|candidate| *candidate == event);
		if events.is_empty() {
			return Err(GatewayAdminError::InvalidQuery);
		}
	}
	Ok(events.into_iter().map(str::to_owned).collect())
}

impl From<AuditRow> for GatewayAuditEntryResponse {
	fn from(row: AuditRow) -> Self {
		let metadata = row.metadata.as_ref().map_or(Value::Null, redact_metadata);
		let reason = metadata.get("reason").and_then(Value::as_str).map(str::to_owned);
		Self {
			id: row.id,
			category: category_of(&row.event).to_owned(),
			event: row.event,
			actor_id: row.actor_id,
			actor_username: row.actor_username,
			target_type: row.target_type,
			target_id: row.target_id,
			target_label: row.target_label,
			reason,
			before: metadata.get("before").cloned(),
			after: metadata.get("after").cloned(),
			metadata,
			created_at: row.created_at,
		}
	}
}

impl GatewayAuditEntryResponse {
	/// Lists gateway-related audit entries, newest first, with redacted metadata.
	///
	/// Only key, limit, budget, model-policy and access events are returned.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::InvalidQuery`] for an unknown category or event, or a storage error.
	pub async fn list(pool: &PgPool, query: &GatewayAuditQuery, page: &PageQuery) -> Result<Page<Self>, GatewayAdminError> {
		let events = audited_events(query)?;
		let pagination = Self::pagination(page);
		let rows = sqlx::query_as::<_, AuditRow>(
			r#"
			SELECT a.id, a.event, a.actor_id, u.username AS actor_username, a.target_type, a.target_id,
			       COALESCE(k.name, t.name) AS target_label, a.metadata, a.created_at
			FROM audit_logs a
			LEFT JOIN users u ON u.id = a.actor_id
			LEFT JOIN gateway_api_keys k ON a.target_type = 'gateway_api_key' AND k.id = a.target_id
			LEFT JOIN teams t ON a.target_type = 'team' AND t.id = a.target_id
			WHERE a.event = ANY($1)
			  AND ($2::uuid IS NULL OR a.actor_id = $2)
			  AND ($3::uuid IS NULL OR a.target_id = $3)
			  AND ($4::timestamptz IS NULL OR a.created_at >= $4)
			  AND ($5::timestamptz IS NULL OR a.created_at <= $5)
			ORDER BY a.created_at DESC, a.id
			LIMIT $6 OFFSET $7
			"#,
		)
		.bind(&events)
		.bind(query.actor_id)
		.bind(query.target_id)
		.bind(query.from)
		.bind(query.to)
		.bind(pagination.limit)
		.bind(pagination.offset)
		.fetch_all(pool)
		.await?;
		let has_more = rows.len() > pagination.page_size;
		Ok(Page {
			has_more,
			items: rows.into_iter().take(pagination.page_size).map(Self::from).collect(),
		})
	}
}

fn percent_used(spent: Decimal, amount: Decimal) -> Option<Decimal> {
	(amount > Decimal::ZERO).then(|| (spent * Decimal::ONE_HUNDRED / amount).round_dp(2))
}

fn budget_event(subject_type: &str, subject: (Uuid, String, Option<Uuid>), amount: Decimal, spent: Decimal, threshold: Decimal) -> Option<GatewayEventResponse> {
	let percent = percent_used(spent, amount).filter(|percent| *percent >= threshold)?;
	Some(GatewayEventResponse {
		kind: EVENT_BUDGET_THRESHOLD.to_owned(),
		subject_type: subject_type.to_owned(),
		subject_id: subject.0,
		subject_label: subject.1,
		team_id: subject.2,
		expires_at: None,
		amount: Some(amount),
		spent: Some(spent),
		percent_used: Some(percent),
	})
}

impl GatewayEventResponse {
	/// Computes the requested operational events from current key, team and budget state.
	///
	/// Expiry events list active keys expiring within the window. Budget events list keys and
	/// teams whose budget usage has reached the threshold. Nothing is delivered; callers poll this
	/// query or forward its rows to a webhook.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::InvalidQuery`] for an unknown kind or an out-of-range window or threshold, or a storage error.
	pub async fn list(pool: &PgPool, query: &GatewayEventQuery) -> Result<Vec<Self>, GatewayAdminError> {
		let days = query.days.unwrap_or(DEFAULT_EVENT_WINDOW_DAYS);
		let threshold = query.threshold_percent.unwrap_or(DEFAULT_BUDGET_THRESHOLD_PERCENT);
		if !(1..=MAX_EVENT_WINDOW_DAYS).contains(&days) || !(1..=100).contains(&threshold) {
			return Err(GatewayAdminError::InvalidQuery);
		}
		let kind = query.kind.as_deref().filter(|kind| !kind.is_empty());
		if kind.is_some_and(|kind| kind != EVENT_KEY_EXPIRING && kind != EVENT_BUDGET_THRESHOLD) {
			return Err(GatewayAdminError::InvalidQuery);
		}
		let mut events = Vec::new();
		if kind.is_none_or(|kind| kind == EVENT_KEY_EXPIRING) {
			events.extend(Self::expiring_keys(pool, days).await?);
		}
		if kind.is_none_or(|kind| kind == EVENT_BUDGET_THRESHOLD) {
			events.extend(Self::budget_thresholds(pool, Decimal::from(threshold)).await?);
		}
		Ok(events)
	}

	async fn expiring_keys(pool: &PgPool, days: i64) -> Result<Vec<Self>, sqlx::Error> {
		let rows = sqlx::query_as::<_, ExpiringKeyRow>(
			r#"
			SELECT id, name, team_id, expires_at
			FROM gateway_api_keys
			WHERE revoked_at IS NULL AND is_enabled AND admin_disabled_at IS NULL AND expires_at > NOW() AND expires_at <= NOW() + make_interval(days => $1::int)
			ORDER BY expires_at, id
			LIMIT $2
			"#,
		)
		.bind(i32::try_from(days).unwrap_or(i32::MAX))
		.bind(MAX_EXPIRING_KEYS)
		.fetch_all(pool)
		.await?;
		Ok(rows
			.into_iter()
			.map(|row| Self {
				kind: EVENT_KEY_EXPIRING.to_owned(),
				subject_type: "key".to_owned(),
				subject_id: row.id,
				subject_label: row.name,
				team_id: Some(row.team_id),
				expires_at: Some(row.expires_at),
				amount: None,
				spent: None,
				percent_used: None,
			})
			.collect())
	}

	async fn budget_thresholds(pool: &PgPool, threshold: Decimal) -> Result<Vec<Self>, sqlx::Error> {
		let mut events = Vec::new();
		let keys = sqlx::query_as::<_, BudgetKeyRow>(
			r#"
			SELECT DISTINCT k.id, k.name, k.team_id
			FROM gateway_api_keys k
			JOIN budget_assignments ba ON ba.api_key_id = k.id
			JOIN budgets b ON b.id = ba.budget_id AND b.is_enabled
			WHERE k.revoked_at IS NULL
			ORDER BY k.id
			LIMIT $1
			"#,
		)
		.bind(MAX_EVENT_BUDGET_KEYS)
		.fetch_all(pool)
		.await?;
		for key in keys {
			for budget in Budget::evaluate(pool, BudgetSubject::key(key.id)).await? {
				events.extend(budget_event("key", (key.id, key.name.clone(), Some(key.team_id)), budget.amount, budget.spent, threshold));
			}
		}
		for team in Budget::team_overview(pool).await? {
			for budget in team.budgets {
				events.extend(budget_event("team", (team.team_id, team.team_name.clone(), Some(team.team_id)), budget.budget.amount, budget.spent, threshold));
			}
		}
		events.sort_by_key(|event| std::cmp::Reverse(event.percent_used));
		Ok(events)
	}
}
