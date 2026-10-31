use super::admin::{AdminAudit, write_admin_audit};
use super::admin_keys::team_rules;
use super::keys::GatewayKeyRuleRow;
use super::keys_repository::{RuleColumns, validate_concurrency};
use super::{
	AUDIT_TEAM_TARGET, EVENT_TEAM_ACCESS_UPDATED, EVENT_TEAM_LIMITS_UPDATED, GatewayAdminError, GatewayAdminTeamQuery, GatewayAdminTeamResponse, GatewayKeyRuleResponse,
	SetGatewayTeamAccessRequest, SetGatewayTeamLimitsRequest, normalize_reason,
};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct TeamRow {
	id: Uuid,
	name: String,
	is_default: bool,
	gateway_enabled: bool,
	gateway_max_concurrent_requests: Option<i32>,
	key_count: i64,
	active_key_count: i64,
}

impl BaseType for GatewayAdminTeamResponse {}

macro_rules! select_teams {
	() => {
		r#"
	SELECT t.id, t.name, t.is_default, t.gateway_enabled, t.gateway_max_concurrent_requests,
	       (SELECT COUNT(*) FROM gateway_api_keys k WHERE k.team_id = t.id) AS key_count,
	       (SELECT COUNT(*) FROM gateway_api_keys k
	        WHERE k.team_id = t.id AND k.revoked_at IS NULL AND k.is_enabled AND k.admin_disabled_at IS NULL AND (k.expires_at IS NULL OR k.expires_at > NOW())) AS active_key_count
	FROM teams t
"#
	};
}

fn rules_snapshot(rows: &[GatewayKeyRuleRow]) -> Value {
	Value::Array(
		rows.iter()
			.map(|row| json!({ "route_class": row.route_class, "metric": row.metric, "capacity": row.capacity, "refill_period_seconds": row.refill_period_seconds }))
			.collect(),
	)
}

async fn stored_rules(tx: &mut Transaction<'_, Postgres>, team_id: &Uuid) -> Result<Vec<GatewayKeyRuleRow>, sqlx::Error> {
	sqlx::query_as::<_, GatewayKeyRuleRow>(
		"SELECT id, route_class, metric, capacity, refill_period_seconds FROM gateway_rate_limit_rules
		 WHERE team_id = $1 AND is_enabled ORDER BY route_class, metric, refill_period_seconds",
	)
	.bind(team_id)
	.fetch_all(&mut **tx)
	.await
}

async fn lock_team_limits(tx: &mut Transaction<'_, Postgres>, team_id: &Uuid) -> Result<Option<i32>, GatewayAdminError> {
	sqlx::query_scalar::<_, Option<i32>>("SELECT gateway_max_concurrent_requests FROM teams WHERE id = $1 FOR UPDATE")
		.bind(team_id)
		.fetch_optional(&mut **tx)
		.await?
		.ok_or(GatewayAdminError::TeamNotFound)
}

fn assemble(row: TeamRow, rules: Vec<GatewayKeyRuleResponse>) -> GatewayAdminTeamResponse {
	GatewayAdminTeamResponse {
		id: row.id,
		name: row.name,
		is_default: row.is_default,
		gateway_enabled: row.gateway_enabled,
		max_concurrent_requests: row.gateway_max_concurrent_requests,
		rules,
		key_count: row.key_count,
		active_key_count: row.active_key_count,
	}
}

impl GatewayAdminTeamResponse {
	/// Lists teams with their gateway switches, limits and key counts, one page at a time.
	///
	/// # Errors
	///
	/// Returns an error when teams, rules or keys cannot be queried.
	pub async fn list(pool: &PgPool, query: &GatewayAdminTeamQuery) -> Result<PaginatedResponse<Self>, sqlx::Error> {
		let pagination = Self::pagination(query.page.unwrap_or(1), query.size.unwrap_or(0));
		let pattern = query
			.search
			.as_deref()
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.map(|value| format!("%{}%", value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
		let rows = sqlx::query_as::<_, TeamRow>(concat!(select_teams!(), " WHERE ($1::text IS NULL OR t.name ILIKE $1) ORDER BY t.name, t.id LIMIT $2 OFFSET $3"))
			.bind(pattern)
			.bind(pagination.limit)
			.bind(pagination.offset)
			.fetch_all(pool)
			.await?;
		let has_more = rows.len() > pagination.page_size;
		let rows = rows.into_iter().take(pagination.page_size).collect::<Vec<_>>();
		let team_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
		let mut rules = team_rules(pool, &team_ids).await?;
		let items = rows
			.into_iter()
			.map(|row| {
				let team_rules = rules.remove(&row.id).unwrap_or_default();
				assemble(row, team_rules)
			})
			.collect();
		Ok(PaginatedResponse { has_more, items })
	}

	/// Finds one team with its gateway switches and limits.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::TeamNotFound`] when the team does not exist, or a storage error.
	pub async fn find(pool: &PgPool, team_id: &Uuid) -> Result<Self, GatewayAdminError> {
		let row = sqlx::query_as::<_, TeamRow>(concat!(select_teams!(), " WHERE t.id = $1"))
			.bind(team_id)
			.fetch_optional(pool)
			.await?
			.ok_or(GatewayAdminError::TeamNotFound)?;
		let rules = team_rules(pool, &[row.id]).await?.remove(&row.id).unwrap_or_default();
		Ok(assemble(row, rules))
	}

	/// Replaces a team's rate rules and concurrency cap and audits the change in the same transaction.
	///
	/// The gateway reads team policy on every authentication, so the change applies to the next request.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::TeamNotFound`], a rule, concurrency or reason validation error, or a storage error.
	pub async fn replace_limits(pool: &PgPool, actor_id: &Uuid, team_id: &Uuid, request: &SetGatewayTeamLimitsRequest) -> Result<Self, GatewayAdminError> {
		let reason = normalize_reason(request.reason.as_deref())?;
		validate_concurrency(request.max_concurrent_requests)?;
		let columns = RuleColumns::validated(&request.rules)?;
		let mut tx = pool.begin().await?;
		let before_concurrency = lock_team_limits(&mut tx, team_id).await?;
		let before_rules = stored_rules(&mut tx, team_id).await?;
		sqlx::query("DELETE FROM gateway_rate_limit_rules WHERE team_id = $1").bind(team_id).execute(&mut *tx).await?;
		let after_rules = sqlx::query_as::<_, GatewayKeyRuleRow>(
			"INSERT INTO gateway_rate_limit_rules (team_id, route_class, metric, capacity, refill_period_seconds)
			 SELECT $1, route_class, metric, capacity, refill_period_seconds FROM UNNEST($2::text[], $3::text[], $4::bigint[], $5::int[])
			 AS r(route_class, metric, capacity, refill_period_seconds)
			 RETURNING id, route_class, metric, capacity, refill_period_seconds",
		)
		.bind(team_id)
		.bind(&columns.classes)
		.bind(&columns.metrics)
		.bind(&columns.capacities)
		.bind(&columns.periods)
		.fetch_all(&mut *tx)
		.await?;
		sqlx::query("UPDATE teams SET gateway_max_concurrent_requests = $2, updated_at = NOW() WHERE id = $1")
			.bind(team_id)
			.bind(request.max_concurrent_requests)
			.execute(&mut *tx)
			.await?;
		let metadata = json!({
			"before": { "rules": rules_snapshot(&before_rules), "max_concurrent_requests": before_concurrency },
			"after": { "rules": rules_snapshot(&after_rules), "max_concurrent_requests": request.max_concurrent_requests },
		});
		let entry = AdminAudit {
			event: EVENT_TEAM_LIMITS_UPDATED,
			actor_id,
			target_type: AUDIT_TEAM_TARGET,
			target_id: team_id,
			reason: reason.as_deref(),
		};
		write_admin_audit(&mut tx, &entry, metadata).await?;
		tx.commit().await?;
		Self::find(pool, team_id).await
	}

	/// Turns gateway access on or off for a team and audits the change in the same transaction.
	///
	/// While access is off, every key of the team fails authentication.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::TeamNotFound`], [`GatewayAdminError::ReasonInvalid`] or a storage error.
	pub async fn set_access(pool: &PgPool, actor_id: &Uuid, team_id: &Uuid, request: &SetGatewayTeamAccessRequest) -> Result<Self, GatewayAdminError> {
		let reason = normalize_reason(request.reason.as_deref())?;
		let mut tx = pool.begin().await?;
		let before: bool = sqlx::query_scalar("SELECT gateway_enabled FROM teams WHERE id = $1 FOR UPDATE")
			.bind(team_id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(GatewayAdminError::TeamNotFound)?;
		sqlx::query("UPDATE teams SET gateway_enabled = $2, updated_at = NOW() WHERE id = $1")
			.bind(team_id)
			.bind(request.enabled)
			.execute(&mut *tx)
			.await?;
		let metadata = json!({ "before": { "gateway_enabled": before }, "after": { "gateway_enabled": request.enabled } });
		let entry = AdminAudit {
			event: EVENT_TEAM_ACCESS_UPDATED,
			actor_id,
			target_type: AUDIT_TEAM_TARGET,
			target_id: team_id,
			reason: reason.as_deref(),
		};
		write_admin_audit(&mut tx, &entry, metadata).await?;
		tx.commit().await?;
		Self::find(pool, team_id).await
	}
}
