#[cfg(test)]
mod support {
	use crate::types::*;
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

	pub async fn create_team(pool: &PgPool, user_id: Uuid, allow_all_models: bool) -> TestResult<Uuid> {
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ($1, $2) RETURNING id")
			.bind(format!("Team {}", Uuid::new_v4()))
			.bind(allow_all_models)
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(team_id)
	}

	pub fn create_request(team_id: Uuid) -> CreateGatewayKeyRequest {
		CreateGatewayKeyRequest {
			name: "Backend".to_string(),
			team_id,
			scopes: vec!["inference:read".to_string(), "inference:write".to_string()],
			expires_at: None,
			max_concurrent_requests: None,
		}
	}

	pub async fn create_key(pool: &PgPool, user_id: Uuid, team_id: Uuid) -> TestResult<(GatewayApiKey, String)> {
		Ok(GatewayApiKey::create(pool, &user_id, &create_request(team_id)).await?)
	}

	pub async fn audit_events(pool: &PgPool, key_id: Uuid) -> TestResult<Vec<String>> {
		Ok(
			sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type = 'gateway_api_key' AND target_id = $1 ORDER BY created_at, id")
				.bind(key_id)
				.fetch_all(pool)
				.await?,
		)
	}

	pub fn rule(route_class: GatewayRouteClass, metric: GatewayRuleMetric, capacity: u64, refill_period_seconds: u32) -> GatewayKeyRuleRequest {
		GatewayKeyRuleRequest {
			route_class,
			metric,
			capacity,
			refill_period_seconds,
		}
	}
}

#[cfg(test)]
mod requests {
	use crate::types::*;

	#[test]
	fn update_distinguishes_null_from_absent() -> Result<(), serde_json::Error> {
		let absent: UpdateGatewayKeyRequest = serde_json::from_str(r#"{"name":"x"}"#)?;
		assert!(absent.expires_at.is_none());
		assert!(absent.max_concurrent_requests.is_none());
		let cleared: UpdateGatewayKeyRequest = serde_json::from_str(r#"{"expires_at":null,"max_concurrent_requests":null}"#)?;
		assert_eq!(cleared.expires_at, Some(None));
		assert_eq!(cleared.max_concurrent_requests, Some(None));
		Ok(())
	}

	#[test]
	fn response_never_contains_secret_material() -> Result<(), serde_json::Error> {
		let key = GatewayApiKey {
			id: uuid::Uuid::new_v4(),
			name: "Key".to_string(),
			team_id: uuid::Uuid::new_v4(),
			key_prefix: "oxc_abc".to_string(),
			last_four: "wxyz".to_string(),
			scopes: sqlx::types::Json(vec!["inference:read".to_string()]),
			is_enabled: true,
			admin_disabled: false,
			expires_at: None,
			revoked_at: None,
			last_used_at: None,
			max_concurrent_requests: None,
			created_at: chrono::Utc::now(),
		};
		let value = serde_json::to_value(key.to_response())?;
		assert!(value.get("secret_hash").is_none());
		assert!(value.get("token").is_none());
		Ok(())
	}
}

#[cfg(test)]
mod keys {
	use super::support::*;
	use crate::types::*;
	use chrono::{Duration, Utc};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	#[sqlx::test(migrations = "./migrations")]
	async fn create_returns_a_working_token_and_stores_only_a_hash(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, token) = create_key(&pool, user_id, team_id).await?;
		assert!(token.starts_with(&key.key_prefix));
		assert!(token.ends_with(&key.last_four));
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!((context.key_id, context.user_id, context.team_id), (key.id, user_id, team_id));
		let (hash, created_by): (String, Option<Uuid>) = sqlx::query_as("SELECT secret_hash, created_by FROM gateway_api_keys WHERE id = $1")
			.bind(key.id)
			.fetch_one(&pool)
			.await?;
		assert!(!token.contains(&hash) && !hash.contains(&token));
		assert!(hash.starts_with("$argon2"));
		assert_eq!(created_by, Some(user_id));
		assert_eq!(audit_events(&pool, key.id).await?, vec!["gateway_api_key_created"]);
		let metadata: serde_json::Value = sqlx::query_scalar("SELECT metadata FROM audit_logs WHERE target_id = $1")
			.bind(key.id)
			.fetch_one(&pool)
			.await?;
		assert!(!metadata.to_string().contains(&token));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn create_rejects_teams_the_user_is_not_in(pool: PgPool) -> TestResult {
		let owner = create_user(&pool).await?;
		let outsider = create_user(&pool).await?;
		let team_id = create_team(&pool, owner, false).await?;
		let result = GatewayApiKey::create(&pool, &outsider, &create_request(team_id)).await;
		assert!(matches!(result, Err(GatewayKeyError::TeamForbidden)));
		assert!(GatewayApiKey::list_for_user(&pool, &outsider).await?.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn create_validates_input(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let base = || create_request(team_id);
		let cases = [
			(CreateGatewayKeyRequest { name: "   ".to_string(), ..base() }, GatewayKeyError::InvalidName),
			(CreateGatewayKeyRequest { name: "x".repeat(101), ..base() }, GatewayKeyError::InvalidName),
			(CreateGatewayKeyRequest { scopes: Vec::new(), ..base() }, GatewayKeyError::InvalidScopes),
			(CreateGatewayKeyRequest { scopes: vec!["admin".to_string()], ..base() }, GatewayKeyError::InvalidScopes),
			(CreateGatewayKeyRequest { scopes: vec!["*".to_string()], ..base() }, GatewayKeyError::InvalidScopes),
			(
				CreateGatewayKeyRequest { expires_at: Some(Utc::now() - Duration::minutes(1)), ..base() },
				GatewayKeyError::InvalidExpiry,
			),
			(CreateGatewayKeyRequest { max_concurrent_requests: Some(0), ..base() }, GatewayKeyError::InvalidConcurrency),
		];
		for (request, expected) in cases {
			let error = GatewayApiKey::create(&pool, &user_id, &request).await.expect_err("invalid input must be rejected");
			assert_eq!(std::mem::discriminant(&error), std::mem::discriminant(&expected), "unexpected error {error:?}");
		}
		assert!(GatewayApiKey::list_for_user(&pool, &user_id).await?.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn create_deduplicates_scopes(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let mut request = create_request(team_id);
		request.scopes = vec!["inference:read".to_string(), "inference:read".to_string()];
		let (key, _) = GatewayApiKey::create(&pool, &user_id, &request).await?;
		assert_eq!(key.scopes.0, vec!["inference:read"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn users_only_see_and_touch_their_own_keys(pool: PgPool) -> TestResult {
		let owner = create_user(&pool).await?;
		let other = create_user(&pool).await?;
		let team_id = create_team(&pool, owner, false).await?;
		let (key, token) = create_key(&pool, owner, team_id).await?;
		assert_eq!(GatewayApiKey::list_for_user(&pool, &owner).await?.len(), 1);
		assert!(GatewayApiKey::list_for_user(&pool, &other).await?.is_empty());
		let update = UpdateGatewayKeyRequest {
			name: Some("Hijacked".to_string()),
			..Default::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &other, &key.id, &update).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::revoke(&pool, &other, &key.id).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::rotate(&pool, &other, &key.id, None).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::list_rules(&pool, &other, &key.id).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::replace_rules(&pool, &other, &key.id, &[]).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::find_owned(&pool, &other, &Uuid::new_v4()).await, Err(GatewayKeyError::NotFound)));
		GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(audit_events(&pool, key.id).await?, vec!["gateway_api_key_created"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn update_changes_fields_and_audits_before_and_after(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let mut request = create_request(team_id);
		request.expires_at = Some(Utc::now() + Duration::days(30));
		request.max_concurrent_requests = Some(4);
		let (key, token) = GatewayApiKey::create(&pool, &user_id, &request).await?;
		let update = UpdateGatewayKeyRequest {
			name: Some("Renamed".to_string()),
			scopes: Some(vec!["inference:read".to_string()]),
			expires_at: Some(None),
			max_concurrent_requests: Some(Some(9)),
			is_enabled: None,
		};
		let updated = GatewayApiKey::update(&pool, &user_id, &key.id, &update).await?;
		assert_eq!(updated.name, "Renamed");
		assert_eq!(updated.scopes.0, vec!["inference:read"]);
		assert_eq!(updated.expires_at, None);
		assert_eq!(updated.max_concurrent_requests, Some(9));
		assert!(updated.is_enabled);
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(context.allows("inference:read") && !context.allows("inference:write"));
		let metadata: serde_json::Value = sqlx::query_scalar("SELECT metadata FROM audit_logs WHERE target_id = $1 AND event = 'gateway_api_key_updated'")
			.bind(key.id)
			.fetch_one(&pool)
			.await?;
		assert_eq!(metadata["before"]["name"], json!("Backend"));
		assert_eq!(metadata["after"]["name"], json!("Renamed"));
		assert_eq!(metadata["after"]["max_concurrent_requests"], json!(9));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn update_validates_input_and_rejects_revoked_keys(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, _) = create_key(&pool, user_id, team_id).await?;
		let past = UpdateGatewayKeyRequest {
			expires_at: Some(Some(Utc::now() - Duration::minutes(1))),
			..Default::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &user_id, &key.id, &past).await, Err(GatewayKeyError::InvalidExpiry)));
		let zero = UpdateGatewayKeyRequest {
			max_concurrent_requests: Some(Some(0)),
			..Default::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &user_id, &key.id, &zero).await, Err(GatewayKeyError::InvalidConcurrency)));
		GatewayApiKey::revoke(&pool, &user_id, &key.id).await?;
		let rename = UpdateGatewayKeyRequest {
			name: Some("Late".to_string()),
			..Default::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &user_id, &key.id, &rename).await, Err(GatewayKeyError::Revoked)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn revoke_blocks_authentication_and_records_the_actor(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, token) = create_key(&pool, user_id, team_id).await?;
		GatewayCredential::authenticate(&pool, &token).await?;
		let revoked = GatewayApiKey::revoke(&pool, &user_id, &key.id).await?;
		assert!(revoked.revoked_at.is_some());
		let revoked_by: Option<Uuid> = sqlx::query_scalar("SELECT revoked_by FROM gateway_api_keys WHERE id = $1").bind(key.id).fetch_one(&pool).await?;
		assert_eq!(revoked_by, Some(user_id));
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		assert!(matches!(GatewayApiKey::revoke(&pool, &user_id, &key.id).await, Err(GatewayKeyError::Revoked)));
		assert!(matches!(GatewayApiKey::rotate(&pool, &user_id, &key.id, None).await, Err(GatewayKeyError::Revoked)));
		assert_eq!(audit_events(&pool, key.id).await?, vec!["gateway_api_key_created", "gateway_api_key_revoked"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn disabled_and_expired_keys_cannot_authenticate(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, token) = create_key(&pool, user_id, team_id).await?;
		let disable = UpdateGatewayKeyRequest {
			is_enabled: Some(false),
			..Default::default()
		};
		GatewayApiKey::update(&pool, &user_id, &key.id, &disable).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		let enable = UpdateGatewayKeyRequest {
			is_enabled: Some(true),
			..Default::default()
		};
		GatewayApiKey::update(&pool, &user_id, &key.id, &enable).await?;
		GatewayCredential::authenticate(&pool, &token).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1").bind(key.id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn authentication_requires_current_team_membership(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, token) = create_key(&pool, user_id, team_id).await?;
		sqlx::query("DELETE FROM team_members WHERE user_id = $1 AND team_id = $2").bind(user_id).bind(team_id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		assert!(matches!(GatewayApiKey::rotate(&pool, &user_id, &key.id, None).await, Err(GatewayKeyError::TeamForbidden)));
		Ok(())
	}
}

#[cfg(test)]
mod rotation {
	use super::support::*;
	use crate::types::*;
	use chrono::{Duration, Utc};
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn old_and_new_keys_work_during_overlap_then_only_the_new_one(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let mut request = create_request(team_id);
		request.max_concurrent_requests = Some(3);
		let (old, old_token) = GatewayApiKey::create(&pool, &user_id, &request).await?;
		let (new, previous, new_token) = GatewayApiKey::rotate(&pool, &user_id, &old.id, Some(3600)).await?;
		assert_ne!(old.id, new.id);
		assert_ne!(old_token, new_token);
		assert_eq!((new.team_id, new.name.as_str(), new.max_concurrent_requests), (team_id, "Backend", Some(3)));
		assert_eq!(new.scopes.0, old.scopes.0);
		let expiry = previous.expires_at.ok_or("old key must get an expiry")?;
		let remaining = expiry - Utc::now();
		assert!(remaining > Duration::minutes(59) && remaining <= Duration::minutes(60));
		GatewayCredential::authenticate(&pool, &old_token).await?;
		GatewayCredential::authenticate(&pool, &new_token).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1").bind(old.id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &old_token).await, Err(GatewayAuthError::Invalid)));
		GatewayCredential::authenticate(&pool, &new_token).await?;
		assert_eq!(audit_events(&pool, old.id).await?, vec!["gateway_api_key_created", "gateway_api_key_rotated"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rotating_a_disabled_key_gives_a_disabled_replacement(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (old, _) = create_key(&pool, user_id, team_id).await?;
		let disable = UpdateGatewayKeyRequest {
			is_enabled: Some(false),
			..Default::default()
		};
		GatewayApiKey::update(&pool, &user_id, &old.id, &disable).await?;
		let (new, previous, token) = GatewayApiKey::rotate(&pool, &user_id, &old.id, None).await?;
		assert!(!new.is_enabled && !previous.is_enabled);
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn zero_overlap_retires_the_old_key_immediately(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (old, old_token) = create_key(&pool, user_id, team_id).await?;
		let (_, _, new_token) = GatewayApiKey::rotate(&pool, &user_id, &old.id, Some(0)).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &old_token).await, Err(GatewayAuthError::Invalid)));
		GatewayCredential::authenticate(&pool, &new_token).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn default_overlap_is_one_day_and_the_maximum_is_enforced(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (old, _) = create_key(&pool, user_id, team_id).await?;
		for invalid in [-1, 7 * 24 * 60 * 60 + 1] {
			assert!(matches!(GatewayApiKey::rotate(&pool, &user_id, &old.id, Some(invalid)).await, Err(GatewayKeyError::InvalidOverlap)));
		}
		assert_eq!(GatewayApiKey::list_for_user(&pool, &user_id).await?.len(), 1);
		let (_, previous, _) = GatewayApiKey::rotate(&pool, &user_id, &old.id, None).await?;
		let remaining = previous.expires_at.ok_or("old key must get an expiry")? - Utc::now();
		assert!(remaining > Duration::hours(23) && remaining <= Duration::hours(24));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_earlier_expiry_is_never_extended_and_expired_keys_cannot_rotate(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let mut request = create_request(team_id);
		request.expires_at = Some(Utc::now() + Duration::minutes(10));
		let (old, _) = GatewayApiKey::create(&pool, &user_id, &request).await?;
		let (new, previous, _) = GatewayApiKey::rotate(&pool, &user_id, &old.id, Some(3600)).await?;
		assert_eq!(previous.expires_at, old.expires_at);
		assert_eq!(new.expires_at, old.expires_at);
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1").bind(old.id).execute(&pool).await?;
		assert!(matches!(GatewayApiKey::rotate(&pool, &user_id, &old.id, None).await, Err(GatewayKeyError::InvalidExpiry)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rotation_copies_key_rate_rules(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (old, _) = create_key(&pool, user_id, team_id).await?;
		GatewayApiKey::replace_rules(&pool, &user_id, &old.id, &[rule(GatewayRouteClass::Inference, GatewayRuleMetric::Tokens, 500, 60)]).await?;
		let (new, _, token) = GatewayApiKey::rotate(&pool, &user_id, &old.id, None).await?;
		let rules = GatewayApiKey::list_rules(&pool, &user_id, &new.id).await?;
		assert_eq!(rules.len(), 1);
		assert_eq!((rules[0].capacity, rules[0].refill_period_seconds), (500, 60));
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.policy.key_rules.len(), 1);
		assert_eq!(GatewayApiKey::list_rules(&pool, &user_id, &old.id).await?.len(), 1);
		Ok(())
	}
}

#[cfg(test)]
mod limits {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn rules_are_replaced_listed_enforced_and_cleared(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, token) = create_key(&pool, user_id, team_id).await?;
		assert!(GatewayApiKey::list_rules(&pool, &user_id, &key.id).await?.is_empty());
		let rules = [
			rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 10, 60),
			rule(GatewayRouteClass::Inference, GatewayRuleMetric::Tokens, 1000, 60),
			rule(GatewayRouteClass::Utility, GatewayRuleMetric::Requests, 5, 1),
		];
		assert_eq!(GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &rules).await?.len(), 3);
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.policy.key_rules.len(), 3);
		assert!(context.policy.team_rules.is_empty());
		let replaced = GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &rules[..1]).await?;
		assert_eq!(replaced.len(), 1);
		assert_eq!(GatewayApiKey::list_rules(&pool, &user_id, &key.id).await?.len(), 1);
		assert!(GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &[]).await?.is_empty());
		assert!(GatewayApiKey::list_rules(&pool, &user_id, &key.id).await?.is_empty());
		let events = audit_events(&pool, key.id).await?;
		assert_eq!(events.iter().filter(|event| event.as_str() == "gateway_api_key_limits_updated").count(), 3);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_rules_are_rejected_without_changing_stored_rules(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, _) = create_key(&pool, user_id, team_id).await?;
		let good = [rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 10, 60)];
		GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &good).await?;
		let utility_tokens = [rule(GatewayRouteClass::Utility, GatewayRuleMetric::Tokens, 10, 60)];
		let zero_capacity = [rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 0, 60)];
		let zero_window = [rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 10, 0)];
		let oversized = [rule(GatewayRouteClass::Inference, GatewayRuleMetric::Tokens, u64::MAX, 60)];
		for invalid in [&utility_tokens[..], &zero_capacity[..], &zero_window[..], &oversized[..]] {
			let result = GatewayApiKey::replace_rules(&pool, &user_id, &key.id, invalid).await;
			assert!(matches!(result, Err(GatewayKeyError::InvalidRule)));
		}
		let duplicates = [
			rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 10, 60),
			rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 20, 60),
		];
		assert!(matches!(
			GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &duplicates).await,
			Err(GatewayKeyError::DuplicateRule)
		));
		let stored = GatewayApiKey::list_rules(&pool, &user_id, &key.id).await?;
		assert_eq!((stored.len(), stored[0].capacity), (1, 10));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn revoked_keys_reject_rule_changes_but_stay_readable(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, false).await?;
		let (key, _) = create_key(&pool, user_id, team_id).await?;
		let good = [rule(GatewayRouteClass::Inference, GatewayRuleMetric::Requests, 10, 60)];
		GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &good).await?;
		GatewayApiKey::revoke(&pool, &user_id, &key.id).await?;
		assert!(matches!(GatewayApiKey::replace_rules(&pool, &user_id, &key.id, &[]).await, Err(GatewayKeyError::Revoked)));
		assert_eq!(GatewayApiKey::list_rules(&pool, &user_id, &key.id).await?.len(), 1);
		Ok(())
	}
}

#[cfg(test)]
mod teams {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn lists_only_the_callers_teams_with_their_models(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let other = create_user(&pool).await?;
		let open_team = create_team(&pool, user_id, true).await?;
		let closed_team = create_team(&pool, user_id, false).await?;
		create_team(&pool, other, true).await?;
		let provider_id: uuid::Uuid = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', 'Acme', 'https://example.com', true) RETURNING id")
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'fast', 'fast', true), ($1, 'off', 'off', false)")
			.bind(provider_id)
			.execute(&pool)
			.await?;
		let teams = GatewayTeamResponse::list_for_user(&pool, &user_id).await?;
		assert_eq!(teams.len(), 2);
		let open = teams.iter().find(|team| team.id == open_team).ok_or("missing open team")?;
		let closed = teams.iter().find(|team| team.id == closed_team).ok_or("missing closed team")?;
		assert_eq!(open.models, vec!["acme/fast"]);
		assert!(closed.models.is_empty());
		sqlx::query("INSERT INTO team_model_access (team_id, provider_id) VALUES ($1, $2)").bind(closed_team).bind(provider_id).execute(&pool).await?;
		let teams = GatewayTeamResponse::list_for_user(&pool, &user_id).await?;
		let closed = teams.iter().find(|team| team.id == closed_team).ok_or("missing closed team")?;
		assert_eq!(closed.models, vec!["acme/fast"]);
		Ok(())
	}
}
