use super::keys::{GatewayKeyRuleRow, GatewayTeamKeyRow, GatewayTeamModelRow};
use super::{
	AuditEntry, CreateGatewayKeyRequest, DEFAULT_ROTATION_OVERLAP_SECONDS, GatewayApiKey, GatewayKeyBudgetResponse, GatewayKeyError, GatewayKeyRuleRequest, GatewayKeyRuleResponse, GatewayManagedTeamResponse,
	GatewayModelResponse, GatewayRatePolicy, GatewayRouteClass, GatewayRuleMetric, GatewayTeamKeysResponse, GatewayTeamResponse, KEY_SCOPES, KeyActor, MAX_KEY_NAME_LENGTH, MAX_ROTATION_OVERLAP_SECONDS, SetGatewayKeyBudgetRequest,
	UpdateGatewayKeyRequest, public_model_id,
};
use crate::logging::AuditLog;
use crate::types::{Budget, BudgetSubject, Team};
use crate::utils::auth::hash_password;
use chrono::{DateTime, Duration, Utc};
use rand::{Rng, distributions::Alphanumeric, rngs::OsRng};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Postgres, Transaction, types::Json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const SECRET_LENGTH: usize = 48;
const AUDIT_TARGET: &str = "gateway_api_key";
const BUDGET_INTERVALS: [&str; 4] = ["daily", "weekly", "monthly", "total"];
const BUDGET_ACTIONS: [&str; 2] = ["block", "warn"];
const MAX_BUDGET_UNITS: i64 = 999_999_999_999;

macro_rules! manage_filter {
	() => {
		"(gateway_api_keys.user_id = $2 OR (gateway_api_keys.user_id IS NULL AND $3) OR EXISTS (SELECT 1 FROM team_members tm WHERE tm.team_id = gateway_api_keys.team_id AND tm.user_id = $2 AND tm.role = 'admin' AND (gateway_api_keys.user_id IS NULL OR $4)))"
	};
}

macro_rules! key_columns {
	() => {
		"id, name, team_id, user_id, key_prefix, last_four, scopes, is_enabled, (admin_disabled_at IS NOT NULL) AS admin_disabled, expires_at, revoked_at, last_used_at, max_concurrent_requests, created_at"
	};
}

// Internal Types

struct IssuedSecret {
	id: Uuid,
	token: String,
	hash: String,
	prefix: String,
	last_four: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum KeyAccess {
	Manage,
	Revoke,
}

struct NewKey<'a> {
	user_id: Option<&'a Uuid>,
	created_by: &'a Uuid,
	team_id: &'a Uuid,
	name: &'a str,
	scopes: &'a [String],
	expires_at: Option<DateTime<Utc>>,
	max_concurrent_requests: Option<i32>,
	is_enabled: bool,
}

pub(super) struct RuleColumns {
	pub classes: Vec<&'static str>,
	pub metrics: Vec<&'static str>,
	pub capacities: Vec<i64>,
	pub periods: Vec<i32>,
}

impl RuleColumns {
	pub(super) fn validated(rules: &[GatewayKeyRuleRequest]) -> Result<Self, GatewayKeyError> {
		let stored = rules.iter().map(GatewayKeyRuleRequest::to_rule).collect::<Vec<_>>();
		GatewayRatePolicy::from_stored(Vec::new(), stored, None, None).map_err(|_| GatewayKeyError::InvalidRule)?;
		let windows = rules
			.iter()
			.map(|rule| (route_class_str(rule.route_class), metric_str(rule.metric), rule.refill_period_seconds))
			.collect::<HashSet<_>>();
		if windows.len() != rules.len() {
			return Err(GatewayKeyError::DuplicateRule);
		}
		Ok(Self {
			classes: rules.iter().map(|rule| route_class_str(rule.route_class)).collect(),
			metrics: rules.iter().map(|rule| metric_str(rule.metric)).collect(),
			capacities: rules.iter().map(|rule| i64::try_from(rule.capacity).unwrap_or(i64::MAX)).collect(),
			periods: rules.iter().map(|rule| i32::try_from(rule.refill_period_seconds).unwrap_or(i32::MAX)).collect(),
		})
	}
}

impl IssuedSecret {
	async fn generate() -> Result<Self, GatewayKeyError> {
		let id = Uuid::new_v4();
		let secret: String = OsRng.sample_iter(&Alphanumeric).take(SECRET_LENGTH).map(char::from).collect();
		let last_four = secret[SECRET_LENGTH - 4..].to_owned();
		let prefix = format!("oxc_{}", id.simple());
		let token = format!("{prefix}_{secret}");
		let hash = tokio::task::spawn_blocking(move || hash_password(&secret))
			.await
			.map_err(|_| GatewayKeyError::Secret)?
			.map_err(|_| GatewayKeyError::Secret)?;
		Ok(Self {
			id,
			token,
			hash,
			prefix,
			last_four,
		})
	}
}

fn validate_name(name: &str) -> Result<&str, GatewayKeyError> {
	let name = name.trim();
	if name.is_empty() || name.chars().count() > MAX_KEY_NAME_LENGTH {
		return Err(GatewayKeyError::InvalidName);
	}
	Ok(name)
}

fn validate_scopes(scopes: &[String]) -> Result<Vec<String>, GatewayKeyError> {
	let mut unique: Vec<String> = Vec::with_capacity(scopes.len());
	for scope in scopes {
		if !KEY_SCOPES.contains(&scope.as_str()) {
			return Err(GatewayKeyError::InvalidScopes);
		}
		if !unique.contains(scope) {
			unique.push(scope.clone());
		}
	}
	if unique.is_empty() {
		return Err(GatewayKeyError::InvalidScopes);
	}
	Ok(unique)
}

fn validate_expiry(expires_at: Option<DateTime<Utc>>) -> Result<(), GatewayKeyError> {
	match expires_at {
		Some(value) if value <= Utc::now() => Err(GatewayKeyError::InvalidExpiry),
		_ => Ok(()),
	}
}

pub(super) fn validate_concurrency(value: Option<i32>) -> Result<(), GatewayKeyError> {
	match value {
		Some(value) if value <= 0 => Err(GatewayKeyError::InvalidConcurrency),
		_ => Ok(()),
	}
}

fn validate_budget(request: &SetGatewayKeyBudgetRequest) -> Result<(), GatewayKeyError> {
	let valid = request.amount > Decimal::ZERO
		&& request.amount <= Decimal::new(MAX_BUDGET_UNITS, 4)
		&& BUDGET_INTERVALS.contains(&request.interval.as_str())
		&& BUDGET_ACTIONS.contains(&request.on_exceed.as_str());
	if valid { Ok(()) } else { Err(GatewayKeyError::InvalidBudget) }
}

pub(super) fn route_class_str(class: GatewayRouteClass) -> &'static str {
	match class {
		GatewayRouteClass::Inference => "inference",
		GatewayRouteClass::Utility => "utility",
	}
}

pub(super) fn metric_str(metric: GatewayRuleMetric) -> &'static str {
	match metric {
		GatewayRuleMetric::Requests => "requests",
		GatewayRuleMetric::Tokens => "tokens",
	}
}

fn key_snapshot(key: &GatewayApiKey) -> Value {
	json!({
		"name": key.name,
		"team_id": key.team_id,
		"service_account": key.user_id.is_none(),
		"scopes": key.scopes.0,
		"is_enabled": key.is_enabled,
		"expires_at": key.expires_at,
		"max_concurrent_requests": key.max_concurrent_requests,
	})
}

async fn write_audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, key_id: &Uuid, metadata: Value) -> Result<(), sqlx::Error> {
	let entry = AuditEntry {
		event,
		actor_id,
		target_type: AUDIT_TARGET,
		target_id: key_id,
		metadata,
	};
	AuditLog::write(tx, entry).await
}

async fn is_team_member(connection: &mut PgConnection, user_id: &Uuid, team_id: &Uuid) -> Result<bool, sqlx::Error> {
	sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM team_members WHERE user_id = $1 AND team_id = $2)")
		.bind(user_id)
		.bind(team_id)
		.fetch_one(connection)
		.await
}

async fn can_manage_team_keys(connection: &mut PgConnection, actor: &KeyActor, team_id: &Uuid) -> Result<bool, sqlx::Error> {
	let team_exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM teams WHERE id = $1)").bind(team_id).fetch_one(&mut *connection).await?;
	if !team_exists {
		return Ok(false);
	}
	Ok(actor.installation_admin || Team::is_admin(&mut *connection, team_id, &actor.user_id).await?)
}

impl GatewayApiKey {
	/// Lists every key owned by the user, newest first.
	///
	/// # Errors
	///
	/// Returns an error when key storage cannot be queried.
	pub async fn list_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", key_columns!(), " FROM gateway_api_keys WHERE user_id = $1 ORDER BY created_at DESC, id"))
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	/// Finds one key the actor owns or manages as a team or installation administrator.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`] when the key is missing or the actor cannot manage it.
	pub async fn find_owned(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid) -> Result<Self, GatewayKeyError> {
		let actor = actor.into();
		sqlx::query_as::<_, Self>(concat!("SELECT ", key_columns!(), " FROM gateway_api_keys WHERE id = $1 AND ", manage_filter!()))
			.bind(key_id)
			.bind(actor.user_id)
			.bind(actor.installation_admin)
			.bind(false)
			.fetch_optional(pool)
			.await?
			.ok_or(GatewayKeyError::NotFound)
	}

	async fn lock_active(tx: &mut Transaction<'_, Postgres>, actor: &KeyActor, key_id: &Uuid, access: KeyAccess) -> Result<Self, GatewayKeyError> {
		let key = sqlx::query_as::<_, Self>(concat!("SELECT ", key_columns!(), " FROM gateway_api_keys WHERE id = $1 AND ", manage_filter!(), " FOR UPDATE"))
			.bind(key_id)
			.bind(actor.user_id)
			.bind(actor.installation_admin)
			.bind(access == KeyAccess::Revoke)
			.fetch_optional(&mut **tx)
			.await?
			.ok_or(GatewayKeyError::NotFound)?;
		if key.revoked_at.is_some() {
			return Err(GatewayKeyError::Revoked);
		}
		Ok(key)
	}

	async fn insert(tx: &mut Transaction<'_, Postgres>, new: &NewKey<'_>, issued: &IssuedSecret) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"INSERT INTO gateway_api_keys (id, user_id, created_by, team_id, name, secret_hash, key_prefix, last_four, scopes, expires_at, max_concurrent_requests, is_enabled) ",
			"VALUES ($1, $2, $12, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING ",
			key_columns!()
		))
		.bind(issued.id)
		.bind(new.user_id)
		.bind(new.team_id)
		.bind(new.name)
		.bind(&issued.hash)
		.bind(&issued.prefix)
		.bind(&issued.last_four)
		.bind(Json(new.scopes))
		.bind(new.expires_at)
		.bind(new.max_concurrent_requests)
		.bind(new.is_enabled)
		.bind(new.created_by)
		.fetch_one(&mut **tx)
		.await
	}

	/// Creates a key pinned to a team and returns it with its one-time token.
	///
	/// A personal key belongs to the actor, who must be in the team. A service-account key has no
	/// user, so it outlives its creator; only team admins and installation admins may issue one.
	///
	/// # Errors
	///
	/// Returns a validation error for a bad name, scope list, expiry or concurrency cap,
	/// [`GatewayKeyError::TeamForbidden`] when the user is not in the team,
	/// [`GatewayKeyError::ServiceAccountForbidden`] when the actor cannot administer the team, or a storage error.
	pub async fn create(pool: &PgPool, actor: impl Into<KeyActor>, request: &CreateGatewayKeyRequest) -> Result<(Self, String), GatewayKeyError> {
		let actor = actor.into();
		let name = validate_name(&request.name)?;
		let scopes = validate_scopes(&request.scopes)?;
		validate_expiry(request.expires_at)?;
		validate_concurrency(request.max_concurrent_requests)?;
		let issued = IssuedSecret::generate().await?;
		let mut tx = pool.begin().await?;
		let owner = if request.service_account {
			if !can_manage_team_keys(&mut tx, &actor, &request.team_id).await? {
				return Err(GatewayKeyError::ServiceAccountForbidden);
			}
			None
		} else {
			if !is_team_member(&mut tx, &actor.user_id, &request.team_id).await? {
				return Err(GatewayKeyError::TeamForbidden);
			}
			Some(&actor.user_id)
		};
		let new = NewKey {
			user_id: owner,
			created_by: &actor.user_id,
			team_id: &request.team_id,
			name,
			scopes: &scopes,
			expires_at: request.expires_at,
			max_concurrent_requests: request.max_concurrent_requests,
			is_enabled: true,
		};
		let key = Self::insert(&mut tx, &new, &issued).await?;
		let metadata = json!({ "after": key_snapshot(&key), "key_prefix": key.key_prefix });
		write_audit(&mut tx, "gateway_api_key_created", &actor.user_id, &key.id, metadata).await?;
		tx.commit().await?;
		Ok((key, issued.token))
	}

	/// Applies a partial update to an active key the actor manages.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`], [`GatewayKeyError::Revoked`], a validation error or a storage error.
	pub async fn update(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid, request: &UpdateGatewayKeyRequest) -> Result<Self, GatewayKeyError> {
		let actor = actor.into();
		let name = request.name.as_deref().map(validate_name).transpose()?;
		let scopes = request.scopes.as_deref().map(validate_scopes).transpose()?;
		if let Some(expires_at) = request.expires_at {
			validate_expiry(expires_at)?;
		}
		if let Some(concurrency) = request.max_concurrent_requests {
			validate_concurrency(concurrency)?;
		}
		let mut tx = pool.begin().await?;
		let before = Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Manage).await?;
		if before.admin_disabled && request.is_enabled == Some(true) {
			return Err(GatewayKeyError::AdminDisabled);
		}
		let updated = sqlx::query_as::<_, Self>(concat!(
			"UPDATE gateway_api_keys SET name = $2, is_enabled = $3, scopes = $4, expires_at = $5, max_concurrent_requests = $6 WHERE id = $1 RETURNING ",
			key_columns!()
		))
		.bind(key_id)
		.bind(name.unwrap_or(&before.name))
		.bind(request.is_enabled.unwrap_or(before.is_enabled))
		.bind(Json(scopes.as_deref().unwrap_or(&before.scopes.0)))
		.bind(request.expires_at.unwrap_or(before.expires_at))
		.bind(request.max_concurrent_requests.unwrap_or(before.max_concurrent_requests))
		.fetch_one(&mut *tx)
		.await?;
		let metadata = json!({ "before": key_snapshot(&before), "after": key_snapshot(&updated) });
		write_audit(&mut tx, "gateway_api_key_updated", &actor.user_id, key_id, metadata).await?;
		tx.commit().await?;
		Ok(updated)
	}

	/// Revokes an active key the actor owns, or any key pinned to a team the actor administers.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`], [`GatewayKeyError::Revoked`] or a storage error.
	pub async fn revoke(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid) -> Result<Self, GatewayKeyError> {
		let actor = actor.into();
		let mut tx = pool.begin().await?;
		let before = Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Revoke).await?;
		let revoked = sqlx::query_as::<_, Self>(concat!(
			"UPDATE gateway_api_keys SET revoked_at = NOW(), revoked_by = $2 WHERE id = $1 RETURNING ",
			key_columns!()
		))
		.bind(key_id)
		.bind(actor.user_id)
		.fetch_one(&mut *tx)
		.await?;
		let metadata = json!({ "key_prefix": revoked.key_prefix, "owner_user_id": before.user_id, "service_account": before.user_id.is_none() });
		write_audit(&mut tx, "gateway_api_key_revoked", &actor.user_id, key_id, metadata).await?;
		tx.commit().await?;
		Ok(revoked)
	}

	/// Issues a replacement key and shortens the old key's expiry to the overlap window.
	///
	/// The new key copies the old key's team, scopes, expiry, concurrency cap and key-scoped
	/// rate rules. An old key that already expires sooner keeps its earlier expiry.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::InvalidOverlap`] for an overlap outside 0 to 7 days,
	/// [`GatewayKeyError::InvalidExpiry`] when the old key has already expired,
	/// [`GatewayKeyError::TeamForbidden`] when the owner left the team, or the usual lookup and storage errors.
	pub async fn rotate(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid, overlap_seconds: Option<i64>) -> Result<(Self, Self, String), GatewayKeyError> {
		let actor = actor.into();
		let overlap = overlap_seconds.unwrap_or(DEFAULT_ROTATION_OVERLAP_SECONDS);
		if !(0..=MAX_ROTATION_OVERLAP_SECONDS).contains(&overlap) {
			return Err(GatewayKeyError::InvalidOverlap);
		}
		let issued = IssuedSecret::generate().await?;
		let mut tx = pool.begin().await?;
		let old = Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Manage).await?;
		validate_expiry(old.expires_at)?;
		if old.admin_disabled {
			return Err(GatewayKeyError::AdminDisabled);
		}
		if let Some(owner) = &old.user_id
			&& !is_team_member(&mut tx, owner, &old.team_id).await?
		{
			return Err(GatewayKeyError::TeamForbidden);
		}
		let new = NewKey {
			user_id: old.user_id.as_ref(),
			created_by: &actor.user_id,
			team_id: &old.team_id,
			name: &old.name,
			scopes: &old.scopes.0,
			expires_at: old.expires_at,
			max_concurrent_requests: old.max_concurrent_requests,
			is_enabled: old.is_enabled,
		};
		let replacement = Self::insert(&mut tx, &new, &issued).await?;
		sqlx::query(
			"INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds, is_enabled)
			 SELECT $2, route_class, metric, capacity, refill_period_seconds, is_enabled FROM gateway_rate_limit_rules WHERE api_key_id = $1",
		)
		.bind(key_id)
		.bind(replacement.id)
		.execute(&mut *tx)
		.await?;
		sqlx::query(
			"INSERT INTO budget_assignments (budget_id, api_key_id) SELECT budget_id, $2 FROM budget_assignments WHERE api_key_id = $1",
		)
		.bind(key_id)
		.bind(replacement.id)
		.execute(&mut *tx)
		.await?;
		let overlap_end = Utc::now() + Duration::seconds(overlap);
		let previous = sqlx::query_as::<_, Self>(concat!(
			"UPDATE gateway_api_keys SET expires_at = LEAST(COALESCE(expires_at, $2), $2) WHERE id = $1 RETURNING ",
			key_columns!()
		))
		.bind(key_id)
		.bind(overlap_end)
		.fetch_one(&mut *tx)
		.await?;
		let metadata = json!({
			"new_key_id": replacement.id,
			"new_key_prefix": replacement.key_prefix,
			"overlap_seconds": overlap,
			"previous_expires_at": old.expires_at,
			"expires_at": previous.expires_at,
		});
		write_audit(&mut tx, "gateway_api_key_rotated", &actor.user_id, key_id, metadata).await?;
		tx.commit().await?;
		Ok((replacement, previous, issued.token))
	}

	/// Lists the key-scoped rate rules of a key owned by the user.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`] or a storage error.
	pub async fn list_rules(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid) -> Result<Vec<GatewayKeyRuleResponse>, GatewayKeyError> {
		Self::find_owned(pool, actor, key_id).await?;
		let rows = sqlx::query_as::<_, GatewayKeyRuleRow>(
			"SELECT id, route_class, metric, capacity, refill_period_seconds FROM gateway_rate_limit_rules
			 WHERE api_key_id = $1 AND is_enabled ORDER BY route_class, metric, refill_period_seconds",
		)
		.bind(key_id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(GatewayKeyRuleResponse::from).collect())
	}

	/// Replaces all key-scoped rate rules of an active key; an empty list removes them.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::InvalidRule`] or [`GatewayKeyError::DuplicateRule`] for rules the
	/// limiter cannot enforce, the usual lookup errors, or a storage error.
	pub async fn replace_rules(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid, rules: &[GatewayKeyRuleRequest]) -> Result<Vec<GatewayKeyRuleResponse>, GatewayKeyError> {
		let actor = actor.into();
		let columns = RuleColumns::validated(rules)?;
		let mut tx = pool.begin().await?;
		Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Manage).await?;
		sqlx::query("DELETE FROM gateway_rate_limit_rules WHERE api_key_id = $1").bind(key_id).execute(&mut *tx).await?;
		let rows = sqlx::query_as::<_, GatewayKeyRuleRow>(
			"INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds)
			 SELECT $1, route_class, metric, capacity, refill_period_seconds FROM UNNEST($2::text[], $3::text[], $4::bigint[], $5::int[])
			 AS r(route_class, metric, capacity, refill_period_seconds)
			 RETURNING id, route_class, metric, capacity, refill_period_seconds",
		)
		.bind(key_id)
		.bind(&columns.classes)
		.bind(&columns.metrics)
		.bind(&columns.capacities)
		.bind(&columns.periods)
		.fetch_all(&mut *tx)
		.await?;
		let summary = rows
			.iter()
			.map(|row| json!({ "route_class": row.route_class, "metric": row.metric, "capacity": row.capacity, "refill_period_seconds": row.refill_period_seconds }))
			.collect::<Vec<_>>();
		write_audit(&mut tx, "gateway_api_key_limits_updated", &actor.user_id, key_id, json!({ "rules": summary })).await?;
		tx.commit().await?;
		Ok(rows.into_iter().map(GatewayKeyRuleResponse::from).collect())
	}
}

impl GatewayApiKey {
	/// Returns the spending budget of a key owned by the user, if one is set.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`] or a storage error.
	pub async fn budget(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid) -> Result<Option<GatewayKeyBudgetResponse>, GatewayKeyError> {
		Self::find_owned(pool, actor, key_id).await?;
		let budgets = Budget::evaluate(pool, BudgetSubject::key(*key_id)).await?;
		Ok(budgets.into_iter().next().map(GatewayKeyBudgetResponse::from))
	}

	/// Sets or replaces the spending budget of an active key.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::InvalidBudget`] for an unsupported budget, the usual lookup errors, or a storage error.
	pub async fn replace_budget(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid, request: &SetGatewayKeyBudgetRequest) -> Result<GatewayKeyBudgetResponse, GatewayKeyError> {
		let actor = actor.into();
		validate_budget(request)?;
		let mut tx = pool.begin().await?;
		Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Manage).await?;
		let updated = sqlx::query(
			"UPDATE budgets SET amount = $2, interval = $3::budget_interval, on_exceed = $4::budget_on_exceed, updated_at = NOW()
			 WHERE id = (SELECT budget_id FROM budget_assignments WHERE api_key_id = $1)",
		)
		.bind(key_id)
		.bind(request.amount)
		.bind(&request.interval)
		.bind(&request.on_exceed)
		.execute(&mut *tx)
		.await?;
		if updated.rows_affected() == 0 {
			sqlx::query(
				"WITH created AS (
					INSERT INTO budgets (name, amount, kind, interval, reset_strategy, on_exceed)
					VALUES ($2, $3, 'pooled', $4::budget_interval, 'calendar', $5::budget_on_exceed)
					RETURNING id
				)
				INSERT INTO budget_assignments (budget_id, api_key_id) SELECT id, $1 FROM created",
			)
			.bind(key_id)
			.bind(format!("key:{key_id}"))
			.bind(request.amount)
			.bind(&request.interval)
			.bind(&request.on_exceed)
			.execute(&mut *tx)
			.await?;
		}
		let metadata = json!({ "amount": request.amount, "interval": request.interval, "on_exceed": request.on_exceed });
		write_audit(&mut tx, "gateway_api_key_budget_updated", &actor.user_id, key_id, metadata).await?;
		tx.commit().await?;
		Self::budget(pool, actor, key_id).await?.ok_or(GatewayKeyError::NotFound)
	}

	/// Removes the spending budget of an active key.
	///
	/// # Errors
	///
	/// Returns the usual lookup errors or a storage error.
	pub async fn clear_budget(pool: &PgPool, actor: impl Into<KeyActor>, key_id: &Uuid) -> Result<(), GatewayKeyError> {
		let actor = actor.into();
		let mut tx = pool.begin().await?;
		Self::lock_active(&mut tx, &actor, key_id, KeyAccess::Manage).await?;
		let removed = sqlx::query("DELETE FROM budget_assignments WHERE budget_id IN (SELECT budget_id FROM budget_assignments WHERE api_key_id = $1)").bind(key_id).execute(&mut *tx).await?;
		if removed.rows_affected() > 0 {
			write_audit(&mut tx, "gateway_api_key_budget_cleared", &actor.user_id, key_id, json!({})).await?;
		}
		tx.commit().await?;
		Ok(())
	}
}

impl GatewayTeamResponse {
	/// Lists the user's teams with the models each one grants through the gateway.
	///
	/// # Errors
	///
	/// Returns an error when team or model storage cannot be queried.
	pub async fn list_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		let teams = Team::summaries_for_user(pool, user_id).await?;
		let team_ids = teams.iter().map(|team| team.id).collect::<Vec<_>>();
		let rows = sqlx::query_as::<_, GatewayTeamModelRow>(
			r"
			SELECT DISTINCT t.id AS team_id, p.name AS provider_name, m.model_id
			FROM teams t
			JOIN models m ON m.is_enabled
			JOIN providers p ON p.id = m.provider_id AND p.is_enabled
			WHERE t.id = ANY($1)
			  AND (t.allow_all_models OR EXISTS (
				SELECT 1 FROM team_model_access a WHERE a.team_id = t.id AND (a.model_id = m.id OR a.provider_id = p.id)
			  ))
			ORDER BY p.name, m.model_id
			",
		)
		.bind(&team_ids)
		.fetch_all(pool)
		.await?;
		let mut models: HashMap<Uuid, Vec<String>> = HashMap::with_capacity(teams.len());
		for row in rows {
			models.entry(row.team_id).or_default().push(public_model_id(&row.provider_name, &row.model_id));
		}
		Ok(teams
			.into_iter()
			.map(|team| Self {
				models: models.remove(&team.id).unwrap_or_default(),
				id: team.id,
				name: team.name,
				is_default: team.is_default,
			})
			.collect())
	}
}

impl GatewayModelResponse {
	/// Lists every model the user's teams grant through the gateway, once per model.
	///
	/// Prices are per million tokens and null when the model has neither a reported price nor an override.
	///
	/// # Errors
	///
	/// Returns an error when team or model storage cannot be queried.
	pub async fn list_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		let teams = Team::summaries_for_user(pool, user_id).await?;
		let team_ids = teams.iter().map(|team| team.id).collect::<Vec<_>>();
		let mut models = sqlx::query_as::<_, Self>(
			r"
			WITH granted AS (
				SELECT m.id AS model_id, ARRAY_AGG(t.id ORDER BY t.id) AS team_ids
				FROM teams t
				JOIN models m ON m.is_enabled
				JOIN providers p ON p.id = m.provider_id AND p.is_enabled
				WHERE t.id = ANY($1)
				  AND (t.allow_all_models OR EXISTS (
					SELECT 1 FROM team_model_access a WHERE a.team_id = t.id AND (a.model_id = m.id OR a.provider_id = p.id)
				  ))
				GROUP BY m.id
			)
			SELECT
				m.model_id,
				m.display_name,
				p.name AS provider,
				COALESCE(sys_mc.capabilities, m.capabilities, '[]'::jsonb) AS capabilities,
				COALESCE(sys_mc.input_modalities, m.input_modalities, '[]'::jsonb) AS input_modalities,
				COALESCE(sys_mc.output_modalities, m.output_modalities, '[]'::jsonb) AS output_modalities,
				COALESCE(sys_mc.context_length, m.context_length) AS context_length,
				COALESCE((mpo.pricing->>'input')::numeric, reported.price_input) AS price_input,
				COALESCE((mpo.pricing->>'output')::numeric, reported.price_output) AS price_output,
				g.team_ids
			FROM granted g
			JOIN models m ON m.id = g.model_id
			JOIN providers p ON p.id = m.provider_id
			LEFT JOIN model_configs sys_mc ON sys_mc.model_id = m.id AND sys_mc.owner_id IS NULL
			LEFT JOIN model_pricing_overrides mpo ON mpo.model_id = m.id
			LEFT JOIN LATERAL (
				SELECT MIN(o.price_input)::numeric AS price_input, MIN(o.price_output)::numeric AS price_output
				FROM gateway_catalog_models gcm
				JOIN gateway_model_provider_options o ON o.catalog_model_id = gcm.id
				WHERE gcm.local_model_id = m.id
			) reported ON TRUE
			ORDER BY m.display_name, p.name
			",
		)
		.bind(&team_ids)
		.fetch_all(pool)
		.await?;
		for model in &mut models {
			model.id = public_model_id(&model.provider, &model.model_id);
		}
		Ok(models)
	}
}

impl GatewayTeamKeysResponse {
	/// Lists the teams the actor administers and every key pinned to them.
	///
	/// Installation administrators see every team and every service-account key. Team admins see
	/// all keys of their teams, including members' personal keys.
	///
	/// # Errors
	///
	/// Returns an error when team or key storage cannot be queried.
	pub async fn load(pool: &PgPool, actor: impl Into<KeyActor>) -> Result<Self, sqlx::Error> {
		let actor = actor.into();
		let teams = sqlx::query_as::<_, (Uuid, String)>(
			"SELECT t.id, t.name FROM teams t
			 WHERE $2 OR EXISTS (SELECT 1 FROM team_members tm WHERE tm.team_id = t.id AND tm.user_id = $1 AND tm.role = 'admin')
			 ORDER BY t.is_default DESC, t.name",
		)
		.bind(actor.user_id)
		.bind(actor.installation_admin)
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(|(id, name)| GatewayManagedTeamResponse { id, name })
		.collect::<Vec<_>>();
		let rows = sqlx::query_as::<_, GatewayTeamKeyRow>(concat!(
			"SELECT ",
			key_columns!(),
			", (SELECT t.name FROM teams t WHERE t.id = gateway_api_keys.team_id) AS team_name,
			 (SELECT u.username FROM users u WHERE u.id = gateway_api_keys.user_id) AS owner_username
			 FROM gateway_api_keys
			 WHERE (gateway_api_keys.user_id IS NULL AND $2)
			    OR EXISTS (SELECT 1 FROM team_members tm WHERE tm.team_id = gateway_api_keys.team_id AND tm.user_id = $1 AND tm.role = 'admin')
			 ORDER BY created_at DESC, id"
		))
		.bind(actor.user_id)
		.bind(actor.installation_admin)
		.fetch_all(pool)
		.await?;
		Ok(Self {
			teams,
			keys: rows.iter().map(GatewayTeamKeyRow::to_response).collect(),
		})
	}
}
