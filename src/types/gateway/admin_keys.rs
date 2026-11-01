use super::keys::GatewayKeyRuleRow;
use super::{
	AUDIT_KEY_TARGET, EVENT_KEY_ADMIN_DISABLED, EVENT_KEY_ADMIN_ENABLED, EVENT_KEY_ADMIN_REVOKED, GatewayAdminError, GatewayAdminKeyDetailResponse, GatewayAdminKeyQuery,
	GatewayAdminKeyResponse, GatewayKeyAdminAction, GatewayKeyRuleResponse, normalize_reason,
};
use super::admin::{AdminAudit, write_admin_audit};
use crate::types::{Page, PageQuery};
use crate::types::{BaseType, Budget, BudgetSubject, RequestLog, RequestLogFilter};
use serde_json::json;
use std::collections::HashMap;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const KEY_STATUSES: [&str; 4] = ["active", "disabled", "revoked", "expired"];

macro_rules! select_keys {
	() => {
		r#"
	SELECT * FROM (
		SELECT k.id, k.name, k.team_id, t.name AS team_name, k.user_id, u.username AS owner_username, (k.user_id IS NULL) AS service_account,
		       k.key_prefix, k.last_four, k.scopes,
		       CASE
		           WHEN k.revoked_at IS NOT NULL THEN 'revoked'
		           WHEN k.admin_disabled_at IS NOT NULL OR NOT k.is_enabled THEN 'disabled'
		           WHEN k.expires_at IS NOT NULL AND k.expires_at <= NOW() THEN 'expired'
		           ELSE 'active'
		       END AS status,
		       k.is_enabled, (k.admin_disabled_at IS NOT NULL) AS admin_disabled, k.expires_at, k.revoked_at, k.last_used_at, k.max_concurrent_requests, k.created_at
		FROM gateway_api_keys k
		JOIN teams t ON t.id = k.team_id
		LEFT JOIN users u ON u.id = k.user_id
	) keys
"#
	};
}

impl GatewayKeyAdminAction {
	const fn event(self) -> &'static str {
		match self {
			Self::Revoke => EVENT_KEY_ADMIN_REVOKED,
			Self::Disable => EVENT_KEY_ADMIN_DISABLED,
			Self::Enable => EVENT_KEY_ADMIN_ENABLED,
		}
	}

	const fn update_sql(self) -> &'static str {
		match self {
			Self::Revoke => "UPDATE gateway_api_keys SET revoked_at = NOW(), revoked_by = $2 WHERE id = $1",
			Self::Disable => "UPDATE gateway_api_keys SET admin_disabled_at = NOW(), admin_disabled_by = $2 WHERE id = $1",
			Self::Enable => "UPDATE gateway_api_keys SET admin_disabled_at = NULL, admin_disabled_by = NULL WHERE id = $1",
		}
	}
}

#[derive(sqlx::FromRow)]
struct LockedKey {
	admin_disabled: bool,
	revoked_at: Option<chrono::DateTime<chrono::Utc>>,
	key_prefix: String,
	user_id: Option<Uuid>,
	team_id: Uuid,
}

impl BaseType for GatewayAdminKeyResponse {}

fn like_pattern(search: Option<&str>) -> Option<String> {
	search
		.map(str::trim)
		.filter(|value| !value.is_empty())
		.map(|value| format!("%{}%", value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")))
}

impl GatewayAdminKeyResponse {
	/// Lists the keys of every user and team, newest first, one page at a time.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::InvalidQuery`] for an unknown status or a storage error.
	pub async fn list(pool: &PgPool, query: &GatewayAdminKeyQuery, page: &PageQuery) -> Result<Page<Self>, GatewayAdminError> {
		let status = query.status.as_deref().filter(|status| !status.is_empty());
		if status.is_some_and(|status| !KEY_STATUSES.contains(&status)) {
			return Err(GatewayAdminError::InvalidQuery);
		}
		let pagination = Self::pagination(page);
		let rows = sqlx::query_as::<_, Self>(concat!(
			select_keys!(),
			"WHERE ($1::uuid IS NULL OR team_id = $1)
			  AND ($2::uuid IS NULL OR user_id = $2)
			  AND ($3::text IS NULL OR status = $3)
			  AND ($4::timestamptz IS NULL OR expires_at >= $4)
			  AND ($5::timestamptz IS NULL OR expires_at <= $5)
			  AND ($6::timestamptz IS NULL OR last_used_at < $6)
			  AND ($7::bool IS NOT TRUE OR last_used_at IS NULL)
			  AND ($8::text IS NULL OR name ILIKE $8 OR key_prefix ILIKE $8 OR owner_username ILIKE $8 OR team_name ILIKE $8)
			ORDER BY created_at DESC, id
			LIMIT $9 OFFSET $10"
		))
			.bind(query.team_id)
			.bind(query.user_id)
			.bind(status)
			.bind(query.expires_from)
			.bind(query.expires_to)
			.bind(query.last_used_before)
			.bind(query.never_used)
			.bind(like_pattern(query.search.as_deref()))
			.bind(pagination.limit)
			.bind(pagination.offset)
			.fetch_all(pool)
			.await?;
		let has_more = rows.len() > pagination.page_size;
		Ok(Page {
			has_more,
			items: rows.into_iter().take(pagination.page_size).collect(),
		})
	}

	/// Finds one key of any user.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::KeyNotFound`] when the key does not exist, or a storage error.
	pub async fn find(pool: &PgPool, key_id: &Uuid) -> Result<Self, GatewayAdminError> {
		sqlx::query_as::<_, Self>(concat!(select_keys!(), " WHERE id = $1"))
			.bind(key_id)
			.fetch_optional(pool)
			.await?
			.ok_or(GatewayAdminError::KeyNotFound)
	}

	/// Revokes, disables or enables a key and records the action with its reason.
	///
	/// A revoked key can no longer change, and the audit row is written in the same transaction.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::KeyNotFound`], [`GatewayAdminError::KeyRevoked`],
	/// [`GatewayAdminError::ReasonInvalid`] or a storage error.
	pub async fn apply_action(pool: &PgPool, actor_id: &Uuid, key_id: &Uuid, action: GatewayKeyAdminAction, reason: Option<&str>) -> Result<Self, GatewayAdminError> {
		let reason = normalize_reason(reason)?;
		let mut tx = pool.begin().await?;
		let before = lock_key(&mut tx, key_id).await?;
		if before.revoked_at.is_some() {
			return Err(GatewayAdminError::KeyRevoked);
		}
		let update = sqlx::query(action.update_sql()).bind(key_id);
		let update = if action == GatewayKeyAdminAction::Enable { update } else { update.bind(actor_id) };
		update.execute(&mut *tx).await?;
		let after_admin_disabled = match action {
			GatewayKeyAdminAction::Revoke => before.admin_disabled,
			GatewayKeyAdminAction::Disable => true,
			GatewayKeyAdminAction::Enable => false,
		};
		let metadata = json!({
			"key_prefix": before.key_prefix,
			"owner_user_id": before.user_id,
			"team_id": before.team_id,
			"before": { "admin_disabled": before.admin_disabled, "revoked": false },
			"after": { "admin_disabled": after_admin_disabled, "revoked": action == GatewayKeyAdminAction::Revoke },
		});
		let entry = AdminAudit {
			event: action.event(),
			actor_id,
			target_type: AUDIT_KEY_TARGET,
			target_id: key_id,
			reason: reason.as_deref(),
		};
		write_admin_audit(&mut tx, &entry, metadata).await?;
		tx.commit().await?;
		Self::find(pool, key_id).await
	}
}

async fn lock_key(tx: &mut Transaction<'_, Postgres>, key_id: &Uuid) -> Result<LockedKey, GatewayAdminError> {
	sqlx::query_as::<_, LockedKey>("SELECT (admin_disabled_at IS NOT NULL) AS admin_disabled, revoked_at, key_prefix, user_id, team_id FROM gateway_api_keys WHERE id = $1 FOR UPDATE")
		.bind(key_id)
		.fetch_optional(&mut **tx)
		.await?
		.ok_or(GatewayAdminError::KeyNotFound)
}

impl GatewayAdminKeyDetailResponse {
	/// Loads a key with its team and key rate rules, concurrency caps, budgets and usage.
	///
	/// Budgets come from the shared budget evaluation, so they match what the gateway enforces.
	///
	/// # Errors
	///
	/// Returns [`GatewayAdminError::KeyNotFound`] or a storage error.
	pub async fn load(pool: &PgPool, key_id: &Uuid) -> Result<Self, GatewayAdminError> {
		let key = GatewayAdminKeyResponse::find(pool, key_id).await?;
		let (team_gateway_enabled, team_max_concurrent_requests): (bool, Option<i32>) =
			sqlx::query_as("SELECT gateway_enabled, gateway_max_concurrent_requests FROM teams WHERE id = $1")
				.bind(key.team_id)
				.fetch_one(pool)
				.await?;
		let team_rules = team_rules(pool, &[key.team_id]).await?.remove(&key.team_id).unwrap_or_default();
		let key_rules = sqlx::query_as::<_, GatewayKeyRuleRow>(
			"SELECT id, route_class, metric, capacity, refill_period_seconds FROM gateway_rate_limit_rules
			 WHERE api_key_id = $1 AND is_enabled ORDER BY route_class, metric, refill_period_seconds",
		)
		.bind(key_id)
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(GatewayKeyRuleResponse::from)
		.collect();
		let budgets = Budget::evaluate(pool, key.user_id.map_or_else(|| BudgetSubject::service_key(key.team_id, key.id), |user_id| BudgetSubject::user_key(user_id, key.id))).await?;
		let usage = RequestLog::summary(
			pool,
			RequestLogFilter {
				api_key_id: Some(key.id),
				..RequestLogFilter::default()
			},
		)
		.await?;
		Ok(Self {
			key,
			team_gateway_enabled,
			team_max_concurrent_requests,
			team_rules,
			key_rules,
			budgets,
			usage,
		})
	}
}

pub(super) async fn team_rules(pool: &PgPool, team_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<GatewayKeyRuleResponse>>, sqlx::Error> {
	let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, i64, i32)>(
		"SELECT team_id, id, route_class, metric, capacity, refill_period_seconds FROM gateway_rate_limit_rules
		 WHERE team_id = ANY($1) AND is_enabled ORDER BY route_class, metric, refill_period_seconds",
	)
	.bind(team_ids)
	.fetch_all(pool)
	.await?;
	let mut rules: HashMap<Uuid, Vec<GatewayKeyRuleResponse>> = HashMap::with_capacity(team_ids.len());
	for (team_id, id, route_class, metric, capacity, refill_period_seconds) in rows {
		rules.entry(team_id).or_default().push(GatewayKeyRuleResponse {
			id,
			route_class,
			metric,
			capacity,
			refill_period_seconds,
		});
	}
	Ok(rules)
}
