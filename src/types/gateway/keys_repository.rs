use super::keys::{GatewayKeyRuleRow, GatewayTeamModelRow};
use super::{
	CreateGatewayKeyRequest, DEFAULT_ROTATION_OVERLAP_SECONDS, GatewayApiKey, GatewayKeyError, GatewayKeyRuleRequest, GatewayKeyRuleResponse, GatewayRatePolicy,
	GatewayRouteClass, GatewayRuleMetric, GatewayTeamResponse, KEY_SCOPES, MAX_KEY_NAME_LENGTH, MAX_ROTATION_OVERLAP_SECONDS, UpdateGatewayKeyRequest,
};
use crate::types::Team;
use crate::utils::auth::hash_password;
use chrono::{DateTime, Duration, Utc};
use rand::{Rng, distributions::Alphanumeric, rngs::OsRng};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Postgres, Transaction, types::Json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const SECRET_LENGTH: usize = 48;
const AUDIT_TARGET: &str = "gateway_api_key";

macro_rules! key_columns {
	() => {
		"id, name, team_id, key_prefix, last_four, scopes, is_enabled, expires_at, revoked_at, last_used_at, max_concurrent_requests, created_at"
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

struct NewKey<'a> {
	user_id: &'a Uuid,
	team_id: &'a Uuid,
	name: &'a str,
	scopes: &'a [String],
	expires_at: Option<DateTime<Utc>>,
	max_concurrent_requests: Option<i32>,
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

fn validate_concurrency(value: Option<i32>) -> Result<(), GatewayKeyError> {
	match value {
		Some(value) if value <= 0 => Err(GatewayKeyError::InvalidConcurrency),
		_ => Ok(()),
	}
}

fn route_class_str(class: GatewayRouteClass) -> &'static str {
	match class {
		GatewayRouteClass::Inference => "inference",
		GatewayRouteClass::Utility => "utility",
	}
}

fn metric_str(metric: GatewayRuleMetric) -> &'static str {
	match metric {
		GatewayRuleMetric::Requests => "requests",
		GatewayRuleMetric::Tokens => "tokens",
	}
}

fn key_snapshot(key: &GatewayApiKey) -> Value {
	json!({
		"name": key.name,
		"team_id": key.team_id,
		"scopes": key.scopes.0,
		"is_enabled": key.is_enabled,
		"expires_at": key.expires_at,
		"max_concurrent_requests": key.max_concurrent_requests,
	})
}

async fn write_audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, key_id: &Uuid, metadata: Value) -> Result<(), sqlx::Error> {
	sqlx::query("INSERT INTO audit_logs (event, actor_id, target_type, target_id, metadata) VALUES ($1, $2, $3, $4, $5)")
		.bind(event)
		.bind(actor_id)
		.bind(AUDIT_TARGET)
		.bind(key_id)
		.bind(metadata)
		.execute(&mut **tx)
		.await?;
	Ok(())
}

async fn is_team_member(connection: &mut PgConnection, user_id: &Uuid, team_id: &Uuid) -> Result<bool, sqlx::Error> {
	sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM team_members WHERE user_id = $1 AND team_id = $2)")
		.bind(user_id)
		.bind(team_id)
		.fetch_one(connection)
		.await
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

	/// Finds one key owned by the user.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`] when the key is missing or owned by someone else.
	pub async fn find_owned(pool: &PgPool, user_id: &Uuid, key_id: &Uuid) -> Result<Self, GatewayKeyError> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", key_columns!(), " FROM gateway_api_keys WHERE id = $1 AND user_id = $2"))
			.bind(key_id)
			.bind(user_id)
			.fetch_optional(pool)
			.await?
			.ok_or(GatewayKeyError::NotFound)
	}

	async fn lock_active(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, key_id: &Uuid) -> Result<Self, GatewayKeyError> {
		let key = sqlx::query_as::<_, Self>(concat!("SELECT ", key_columns!(), " FROM gateway_api_keys WHERE id = $1 AND user_id = $2 FOR UPDATE"))
			.bind(key_id)
			.bind(user_id)
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
			"INSERT INTO gateway_api_keys (id, user_id, created_by, team_id, name, secret_hash, key_prefix, last_four, scopes, expires_at, max_concurrent_requests) ",
			"VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING ",
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
		.fetch_one(&mut **tx)
		.await
	}

	/// Creates a key pinned to one of the user's teams and returns it with its one-time token.
	///
	/// # Errors
	///
	/// Returns a validation error for a bad name, scope list, expiry or concurrency cap,
	/// [`GatewayKeyError::TeamForbidden`] when the user is not in the team, or a storage error.
	pub async fn create(pool: &PgPool, user_id: &Uuid, request: &CreateGatewayKeyRequest) -> Result<(Self, String), GatewayKeyError> {
		let name = validate_name(&request.name)?;
		let scopes = validate_scopes(&request.scopes)?;
		validate_expiry(request.expires_at)?;
		validate_concurrency(request.max_concurrent_requests)?;
		let issued = IssuedSecret::generate().await?;
		let mut tx = pool.begin().await?;
		if !is_team_member(&mut tx, user_id, &request.team_id).await? {
			return Err(GatewayKeyError::TeamForbidden);
		}
		let new = NewKey {
			user_id,
			team_id: &request.team_id,
			name,
			scopes: &scopes,
			expires_at: request.expires_at,
			max_concurrent_requests: request.max_concurrent_requests,
		};
		let key = Self::insert(&mut tx, &new, &issued).await?;
		let metadata = json!({ "after": key_snapshot(&key), "key_prefix": key.key_prefix });
		write_audit(&mut tx, "gateway_api_key_created", user_id, &key.id, metadata).await?;
		tx.commit().await?;
		Ok((key, issued.token))
	}

	/// Applies a partial update to an active key owned by the user.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`], [`GatewayKeyError::Revoked`], a validation error or a storage error.
	pub async fn update(pool: &PgPool, user_id: &Uuid, key_id: &Uuid, request: &UpdateGatewayKeyRequest) -> Result<Self, GatewayKeyError> {
		let name = request.name.as_deref().map(validate_name).transpose()?;
		let scopes = request.scopes.as_deref().map(validate_scopes).transpose()?;
		if let Some(expires_at) = request.expires_at {
			validate_expiry(expires_at)?;
		}
		if let Some(concurrency) = request.max_concurrent_requests {
			validate_concurrency(concurrency)?;
		}
		let mut tx = pool.begin().await?;
		let before = Self::lock_active(&mut tx, user_id, key_id).await?;
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
		write_audit(&mut tx, "gateway_api_key_updated", user_id, key_id, metadata).await?;
		tx.commit().await?;
		Ok(updated)
	}

	/// Revokes an active key owned by the user.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`], [`GatewayKeyError::Revoked`] or a storage error.
	pub async fn revoke(pool: &PgPool, user_id: &Uuid, key_id: &Uuid) -> Result<Self, GatewayKeyError> {
		let mut tx = pool.begin().await?;
		Self::lock_active(&mut tx, user_id, key_id).await?;
		let revoked = sqlx::query_as::<_, Self>(concat!(
			"UPDATE gateway_api_keys SET revoked_at = NOW(), revoked_by = $2 WHERE id = $1 RETURNING ",
			key_columns!()
		))
		.bind(key_id)
		.bind(user_id)
		.fetch_one(&mut *tx)
		.await?;
		write_audit(&mut tx, "gateway_api_key_revoked", user_id, key_id, json!({ "key_prefix": revoked.key_prefix })).await?;
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
	/// [`GatewayKeyError::TeamForbidden`] when the user left the team, or the usual lookup and storage errors.
	pub async fn rotate(pool: &PgPool, user_id: &Uuid, key_id: &Uuid, overlap_seconds: Option<i64>) -> Result<(Self, Self, String), GatewayKeyError> {
		let overlap = overlap_seconds.unwrap_or(DEFAULT_ROTATION_OVERLAP_SECONDS);
		if !(0..=MAX_ROTATION_OVERLAP_SECONDS).contains(&overlap) {
			return Err(GatewayKeyError::InvalidOverlap);
		}
		let issued = IssuedSecret::generate().await?;
		let mut tx = pool.begin().await?;
		let old = Self::lock_active(&mut tx, user_id, key_id).await?;
		validate_expiry(old.expires_at)?;
		if !is_team_member(&mut tx, user_id, &old.team_id).await? {
			return Err(GatewayKeyError::TeamForbidden);
		}
		let new = NewKey {
			user_id,
			team_id: &old.team_id,
			name: &old.name,
			scopes: &old.scopes.0,
			expires_at: old.expires_at,
			max_concurrent_requests: old.max_concurrent_requests,
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
		write_audit(&mut tx, "gateway_api_key_rotated", user_id, key_id, metadata).await?;
		tx.commit().await?;
		Ok((replacement, previous, issued.token))
	}

	/// Lists the key-scoped rate rules of a key owned by the user.
	///
	/// # Errors
	///
	/// Returns [`GatewayKeyError::NotFound`] or a storage error.
	pub async fn list_rules(pool: &PgPool, user_id: &Uuid, key_id: &Uuid) -> Result<Vec<GatewayKeyRuleResponse>, GatewayKeyError> {
		Self::find_owned(pool, user_id, key_id).await?;
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
	pub async fn replace_rules(pool: &PgPool, user_id: &Uuid, key_id: &Uuid, rules: &[GatewayKeyRuleRequest]) -> Result<Vec<GatewayKeyRuleResponse>, GatewayKeyError> {
		let stored = rules.iter().map(GatewayKeyRuleRequest::to_rule).collect::<Vec<_>>();
		GatewayRatePolicy::from_stored(Vec::new(), stored, None, None).map_err(|_| GatewayKeyError::InvalidRule)?;
		let classes = rules.iter().map(|rule| route_class_str(rule.route_class)).collect::<Vec<_>>();
		let metrics = rules.iter().map(|rule| metric_str(rule.metric)).collect::<Vec<_>>();
		let windows = rules
			.iter()
			.map(|rule| (rule.route_class, rule.metric, rule.refill_period_seconds))
			.map(|(class, metric, period)| (route_class_str(class), metric_str(metric), period))
			.collect::<HashSet<_>>();
		if windows.len() != rules.len() {
			return Err(GatewayKeyError::DuplicateRule);
		}
		let capacities = rules.iter().map(|rule| i64::try_from(rule.capacity).unwrap_or(i64::MAX)).collect::<Vec<_>>();
		let periods = rules.iter().map(|rule| i32::try_from(rule.refill_period_seconds).unwrap_or(i32::MAX)).collect::<Vec<_>>();
		let mut tx = pool.begin().await?;
		Self::lock_active(&mut tx, user_id, key_id).await?;
		sqlx::query("DELETE FROM gateway_rate_limit_rules WHERE api_key_id = $1").bind(key_id).execute(&mut *tx).await?;
		let rows = sqlx::query_as::<_, GatewayKeyRuleRow>(
			"INSERT INTO gateway_rate_limit_rules (api_key_id, route_class, metric, capacity, refill_period_seconds)
			 SELECT $1, route_class, metric, capacity, refill_period_seconds FROM UNNEST($2::text[], $3::text[], $4::bigint[], $5::int[])
			 AS r(route_class, metric, capacity, refill_period_seconds)
			 RETURNING id, route_class, metric, capacity, refill_period_seconds",
		)
		.bind(key_id)
		.bind(&classes)
		.bind(&metrics)
		.bind(&capacities)
		.bind(&periods)
		.fetch_all(&mut *tx)
		.await?;
		let summary = rows
			.iter()
			.map(|row| json!({ "route_class": row.route_class, "metric": row.metric, "capacity": row.capacity, "refill_period_seconds": row.refill_period_seconds }))
			.collect::<Vec<_>>();
		write_audit(&mut tx, "gateway_api_key_limits_updated", user_id, key_id, json!({ "rules": summary })).await?;
		tx.commit().await?;
		Ok(rows.into_iter().map(GatewayKeyRuleResponse::from).collect())
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
			models.entry(row.team_id).or_default().push(format!("{}/{}", row.provider_name.to_ascii_lowercase(), row.model_id));
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
