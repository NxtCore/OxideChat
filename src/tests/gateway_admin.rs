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

	pub async fn create_team(pool: &PgPool, user_id: Uuid) -> TestResult<Uuid> {
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ($1, true) RETURNING id")
			.bind(format!("Team {}", Uuid::new_v4()))
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(team_id).bind(user_id).execute(pool).await?;
		Ok(team_id)
	}

	pub async fn create_key(pool: &PgPool, user_id: Uuid, team_id: Uuid) -> TestResult<(GatewayApiKey, String)> {
		let request = CreateGatewayKeyRequest {
			name: "Backend".to_string(),
			team_id,
			scopes: vec!["inference:read".to_string()],
			expires_at: None,
			max_concurrent_requests: None,
		};
		Ok(GatewayApiKey::create(pool, &user_id, &request).await?)
	}

	pub async fn events_for(pool: &PgPool, target_id: Uuid) -> TestResult<Vec<String>> {
		Ok(sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_id = $1 ORDER BY created_at, id").bind(target_id).fetch_all(pool).await?)
	}

	pub fn limits(rules: Vec<GatewayKeyRuleRequest>, max_concurrent_requests: Option<i32>) -> SetGatewayTeamLimitsRequest {
		SetGatewayTeamLimitsRequest {
			rules,
			max_concurrent_requests,
			reason: Some("capacity".to_string()),
		}
	}

	pub fn rule(capacity: u64) -> GatewayKeyRuleRequest {
		GatewayKeyRuleRequest {
			route_class: GatewayRouteClass::Inference,
			metric: GatewayRuleMetric::Requests,
			capacity,
			refill_period_seconds: 60,
		}
	}
}

#[cfg(test)]
mod redaction {
	use crate::types::*;
	use serde_json::json;

	#[test]
	fn secrets_and_content_are_replaced() {
		let value = json!({
			"secret_hash": "abc",
			"token": "oxc_x",
			"nested": { "prompt": "hi", "Authorization": "Bearer abc", "items": ["oxc_0123_secretsecret", "fine"] },
			"max_tokens": 5,
			"key_prefix": "oxc_0123",
		});
		let redacted = redact_metadata(&value);
		assert_eq!(redacted["secret_hash"], REDACTED_MARKER);
		assert_eq!(redacted["token"], REDACTED_MARKER);
		assert_eq!(redacted["nested"]["prompt"], REDACTED_MARKER);
		assert_eq!(redacted["nested"]["Authorization"], REDACTED_MARKER);
		assert_eq!(redacted["nested"]["items"][0], REDACTED_MARKER);
		assert_eq!(redacted["nested"]["items"][1], "fine");
		assert_eq!(redacted["max_tokens"], 5);
		assert_eq!(redacted["key_prefix"], "oxc_0123");
	}

	#[test]
	fn reason_is_trimmed_and_bounded() -> Result<(), GatewayAdminError> {
		assert_eq!(normalize_reason(Some("  why  "))?, Some("why".to_string()));
		assert_eq!(normalize_reason(Some("   "))?, None);
		assert!(matches!(normalize_reason(Some(&"x".repeat(MAX_ADMIN_REASON_LENGTH + 1))), Err(GatewayAdminError::ReasonInvalid)));
		Ok(())
	}
}

#[cfg(test)]
mod keys {
	use super::support::*;
	use crate::types::*;
	use chrono::{Duration, Utc};
	use sqlx::PgPool;
	use uuid::Uuid;

	#[sqlx::test(migrations = "./migrations")]
	async fn list_filters_across_users_and_teams(pool: PgPool) -> TestResult {
		let first = create_user(&pool).await?;
		let second = create_user(&pool).await?;
		let first_team = create_team(&pool, first).await?;
		let second_team = create_team(&pool, second).await?;
		let (first_key, _) = create_key(&pool, first, first_team).await?;
		let (second_key, _) = create_key(&pool, second, second_team).await?;
		let all = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery::default()).await?;
		assert_eq!(all.items.len(), 2);
		let by_team = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { team_id: Some(second_team), ..Default::default() }).await?;
		assert_eq!(by_team.items.iter().map(|key| key.id).collect::<Vec<_>>(), vec![second_key.id]);
		let by_user = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { user_id: Some(first), ..Default::default() }).await?;
		assert_eq!(by_user.items.iter().map(|key| key.id).collect::<Vec<_>>(), vec![first_key.id]);
		let never_used = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { never_used: Some(true), ..Default::default() }).await?;
		assert_eq!(never_used.items.len(), 2);
		sqlx::query("UPDATE gateway_api_keys SET last_used_at = NOW() WHERE id = $1").bind(first_key.id).execute(&pool).await?;
		let never_used = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { never_used: Some(true), ..Default::default() }).await?;
		assert_eq!(never_used.items.len(), 1);
		let stale = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { last_used_before: Some(Utc::now() + Duration::hours(1)), ..Default::default() }).await?;
		assert_eq!(stale.items.len(), 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_filters_by_status_and_expiry(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (active, _) = create_key(&pool, user, team).await?;
		let (expired, _) = create_key(&pool, user, team).await?;
		let (disabled, _) = create_key(&pool, user, team).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() - INTERVAL '1 day' WHERE id = $1").bind(expired.id).execute(&pool).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() + INTERVAL '2 days' WHERE id = $1").bind(active.id).execute(&pool).await?;
		sqlx::query("UPDATE gateway_api_keys SET is_enabled = FALSE WHERE id = $1").bind(disabled.id).execute(&pool).await?;
		for (status, id) in [("active", active.id), ("expired", expired.id), ("disabled", disabled.id)] {
			let page = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { status: Some(status.to_string()), ..Default::default() }).await?;
			assert_eq!(page.items.iter().map(|key| key.id).collect::<Vec<_>>(), vec![id]);
		}
		let soon = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { expires_to: Some(Utc::now() + Duration::days(3)), expires_from: Some(Utc::now()), ..Default::default() }).await?;
		assert_eq!(soon.items.len(), 1);
		let invalid = GatewayAdminKeyResponse::list(&pool, &GatewayAdminKeyQuery { status: Some("bogus".to_string()), ..Default::default() }).await;
		assert!(matches!(invalid, Err(GatewayAdminError::InvalidQuery)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn responses_never_expose_secret_material(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (key, token) = create_key(&pool, user, team).await?;
		let detail = GatewayAdminKeyDetailResponse::load(&pool, &key.id).await?;
		let text = serde_json::to_string(&detail)?;
		assert!(!text.contains(&token) && !text.contains("secret_hash") && !text.contains("$argon2"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn detail_reports_effective_limits_and_usage(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (key, _) = create_key(&pool, user, team).await?;
		GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(10)], Some(3))).await?;
		GatewayApiKey::replace_rules(&pool, &user, &key.id, &[rule(5)]).await?;
		sqlx::query("INSERT INTO gateway_request_logs (user_id, team_id, api_key_id, public_model_id, request_type, status, input_tokens, output_tokens) VALUES ($1, $2, $3, 'm', 'gateway.chat', 'error', 4, 6)")
			.bind(user)
			.bind(team)
			.bind(key.id)
			.execute(&pool)
			.await?;
		let detail = GatewayAdminKeyDetailResponse::load(&pool, &key.id).await?;
		assert_eq!(detail.team_rules.len(), 1);
		assert_eq!(detail.key_rules.len(), 1);
		assert_eq!(detail.team_max_concurrent_requests, Some(3));
		assert_eq!((detail.usage.total_requests, detail.usage.error_requests, detail.usage.input_tokens), (1, 1, 4));
		let missing = GatewayAdminKeyDetailResponse::load(&pool, &Uuid::new_v4()).await;
		assert!(matches!(missing, Err(GatewayAdminError::KeyNotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_actions_are_audited_and_block_authentication(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (key, token) = create_key(&pool, user, team).await?;
		let disabled = GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Disable, Some("suspicious")).await?;
		assert_eq!(disabled.status, "disabled");
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_err());
		let enabled = GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Enable, None).await?;
		assert_eq!(enabled.status, "active");
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		let revoked = GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Revoke, None).await?;
		assert_eq!(revoked.status, "revoked");
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_err());
		let again = GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Disable, None).await;
		assert!(matches!(again, Err(GatewayAdminError::KeyRevoked)));
		let events = events_for(&pool, key.id).await?;
		assert_eq!(
			events,
			vec!["gateway_api_key_created", "gateway_api_key_admin_disabled", "gateway_api_key_admin_enabled", "gateway_api_key_admin_revoked"]
		);
		let (actor, metadata): (Uuid, serde_json::Value) =
			sqlx::query_as("SELECT actor_id, metadata FROM audit_logs WHERE target_id = $1 AND event = 'gateway_api_key_admin_disabled'").bind(key.id).fetch_one(&pool).await?;
		assert_eq!(actor, admin);
		assert_eq!(metadata["reason"], "suspicious");
		assert!(!metadata.to_string().contains(&token));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_action_errors(pool: PgPool) -> TestResult {
		let admin = create_user(&pool).await?;
		let missing = GatewayAdminKeyResponse::apply_action(&pool, &admin, &Uuid::new_v4(), GatewayKeyAdminAction::Revoke, None).await;
		assert!(matches!(missing, Err(GatewayAdminError::KeyNotFound)));
		let user = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (key, _) = create_key(&pool, user, team).await?;
		let long = "x".repeat(MAX_ADMIN_REASON_LENGTH + 1);
		let invalid = GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Revoke, Some(&long)).await;
		assert!(matches!(invalid, Err(GatewayAdminError::ReasonInvalid)));
		assert_eq!(events_for(&pool, key.id).await?, vec!["gateway_api_key_created"]);
		Ok(())
	}
}

#[cfg(test)]
mod teams {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;
	use uuid::Uuid;

	#[sqlx::test(migrations = "./migrations")]
	async fn limits_replace_rules_and_concurrency_with_audit(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (_, token) = create_key(&pool, user, team).await?;
		let updated = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(10)], Some(4))).await?;
		assert_eq!((updated.rules.len(), updated.max_concurrent_requests, updated.key_count), (1, Some(4), 1));
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.policy.team_concurrency, Some(4));
		assert_eq!(context.policy.team_rules.len(), 1);
		let cleared = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(Vec::new(), None)).await?;
		assert!(cleared.rules.is_empty() && cleared.max_concurrent_requests.is_none());
		let metadata: serde_json::Value = sqlx::query_scalar("SELECT metadata FROM audit_logs WHERE target_id = $1 ORDER BY created_at DESC, id LIMIT 1").bind(team).fetch_one(&pool).await?;
		assert_eq!(metadata["before"]["max_concurrent_requests"], 4);
		assert_eq!(metadata["reason"], "capacity");
		assert_eq!(events_for(&pool, team).await?, vec!["gateway_team_limits_updated", "gateway_team_limits_updated"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_limits_change_nothing(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(10)], Some(2))).await?;
		let zero = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(10)], Some(0))).await;
		assert!(matches!(zero, Err(GatewayAdminError::InvalidConcurrency)));
		let empty_rule = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(0)], None)).await;
		assert!(matches!(empty_rule, Err(GatewayAdminError::InvalidRule)));
		let duplicate = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(1), rule(2)], None)).await;
		assert!(matches!(duplicate, Err(GatewayAdminError::DuplicateRule)));
		let missing = GatewayAdminTeamResponse::replace_limits(&pool, &admin, &Uuid::new_v4(), &limits(Vec::new(), None)).await;
		assert!(matches!(missing, Err(GatewayAdminError::TeamNotFound)));
		let team_state = GatewayAdminTeamResponse::find(&pool, &team).await?;
		assert_eq!((team_state.rules.len(), team_state.max_concurrent_requests), (1, Some(2)));
		assert_eq!(events_for(&pool, team).await?.len(), 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn access_off_blocks_every_key_of_the_team(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (_, token) = create_key(&pool, user, team).await?;
		let off = GatewayAdminTeamResponse::set_access(&pool, &admin, &team, &SetGatewayTeamAccessRequest { enabled: false, reason: None }).await?;
		assert!(!off.gateway_enabled);
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_err());
		let on = GatewayAdminTeamResponse::set_access(&pool, &admin, &team, &SetGatewayTeamAccessRequest { enabled: true, reason: None }).await?;
		assert!(on.gateway_enabled);
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		assert_eq!(events_for(&pool, team).await?, vec!["gateway_team_access_updated", "gateway_team_access_updated"]);
		let missing = GatewayAdminTeamResponse::set_access(&pool, &admin, &Uuid::new_v4(), &SetGatewayTeamAccessRequest { enabled: false, reason: None }).await;
		assert!(matches!(missing, Err(GatewayAdminError::TeamNotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_includes_rules_and_key_counts(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		create_key(&pool, user, team).await?;
		GatewayAdminTeamResponse::replace_limits(&pool, &admin, &team, &limits(vec![rule(7)], None)).await?;
		let page = GatewayAdminTeamResponse::list(&pool, &GatewayAdminTeamQuery { search: Some("Team".to_string()), ..Default::default() }).await?;
		let listed = page.items.iter().find(|item| item.id == team).ok_or("team missing")?;
		assert_eq!((listed.rules.len(), listed.active_key_count), (1, 1));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_policy_changes_are_audited(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team_id = create_team(&pool, user).await?;
		let team = Team::find_by_id(&pool, &team_id).await?.ok_or("team")?;
		team.update_model_access(&pool, &admin, &[], &[]).await?;
		assert_eq!(events_for(&pool, team_id).await?, vec!["team_models_updated"]);
		Ok(())
	}
}

#[cfg(test)]
mod audit_and_events {
	use super::support::*;
	use crate::types::*;
	use chrono::{Duration, Utc};
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn audit_lists_gateway_events_with_redacted_metadata(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let admin = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (key, _) = create_key(&pool, user, team).await?;
		GatewayAdminKeyResponse::apply_action(&pool, &admin, &key.id, GatewayKeyAdminAction::Disable, Some("because")).await?;
		GatewayAdminTeamResponse::set_access(&pool, &admin, &team, &SetGatewayTeamAccessRequest { enabled: false, reason: None }).await?;
		sqlx::query("INSERT INTO audit_logs (event, target_type, target_id, metadata) VALUES ('gateway_api_key_updated', 'gateway_api_key', $1, '{\"before\":{\"secret_hash\":\"abc\"},\"after\":{\"name\":\"n\"}}')")
			.bind(key.id)
			.execute(&pool)
			.await?;
		sqlx::query("INSERT INTO audit_logs (event, actor_id) VALUES ('user_login', $1)").bind(admin).execute(&pool).await?;
		let all = GatewayAuditEntryResponse::list(&pool, &GatewayAuditQuery::default()).await?;
		assert_eq!(all.items.len(), 4);
		assert!(all.items.iter().all(|entry| entry.event != "user_login"));
		let disabled = all.items.iter().find(|entry| entry.event == "gateway_api_key_admin_disabled").ok_or("missing")?;
		assert_eq!((disabled.reason.as_deref(), disabled.actor_id, disabled.category.as_str()), (Some("because"), Some(admin), "key"));
		assert_eq!(disabled.target_label.as_deref(), Some("Backend"));
		assert!(disabled.before.is_some() && disabled.after.is_some());
		let updated = all.items.iter().find(|entry| entry.event == "gateway_api_key_updated" && entry.actor_id.is_none()).ok_or("missing")?;
		assert_eq!(updated.before.as_ref().map(|value| value["secret_hash"].clone()), Some(serde_json::json!(REDACTED_MARKER)));
		let access = GatewayAuditEntryResponse::list(&pool, &GatewayAuditQuery { category: Some("access".to_string()), ..Default::default() }).await?;
		assert_eq!(access.items.len(), 1);
		let by_target = GatewayAuditEntryResponse::list(&pool, &GatewayAuditQuery { target_id: Some(team), ..Default::default() }).await?;
		assert_eq!(by_target.items.len(), 1);
		let future = GatewayAuditEntryResponse::list(&pool, &GatewayAuditQuery { from: Some(Utc::now() + Duration::hours(1)), ..Default::default() }).await?;
		assert!(future.items.is_empty());
		let bad = GatewayAuditEntryResponse::list(&pool, &GatewayAuditQuery { category: Some("nope".to_string()), ..Default::default() }).await;
		assert!(matches!(bad, Err(GatewayAdminError::InvalidQuery)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn events_report_expiring_keys_and_budget_thresholds(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let team = create_team(&pool, user).await?;
		let (soon, _) = create_key(&pool, user, team).await?;
		let (later, _) = create_key(&pool, user, team).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() + INTERVAL '2 days' WHERE id = $1").bind(soon.id).execute(&pool).await?;
		sqlx::query("UPDATE gateway_api_keys SET expires_at = NOW() + INTERVAL '30 days' WHERE id = $1").bind(later.id).execute(&pool).await?;
		let expiring = GatewayEventResponse::list(&pool, &GatewayEventQuery { kind: Some("key_expiring".to_string()), ..Default::default() }).await?;
		assert_eq!(expiring.iter().map(|event| event.subject_id).collect::<Vec<_>>(), vec![soon.id]);
		GatewayApiKey::replace_budget(
			&pool,
			&user,
			&soon.id,
			&SetGatewayKeyBudgetRequest {
				amount: rust_decimal::Decimal::new(10, 0),
				interval: "total".to_string(),
				on_exceed: "block".to_string(),
			},
		)
		.await?;
		sqlx::query("INSERT INTO usage_events (user_id, team_id, api_key_id, request_type, cost_total) VALUES ($1, $2, $3, 'chat', 9)")
			.bind(user)
			.bind(team)
			.bind(soon.id)
			.execute(&pool)
			.await?;
		let query = GatewayEventQuery {
			kind: Some("budget_threshold".to_string()),
			threshold_percent: Some(50),
			..Default::default()
		};
		let budget_events = GatewayEventResponse::list(&pool, &query).await?;
		assert_eq!(budget_events.len(), 1);
		assert_eq!((budget_events[0].subject_id, budget_events[0].percent_used), (soon.id, Some(rust_decimal::Decimal::new(90, 0))));
		let query = GatewayEventQuery {
			kind: Some("budget_threshold".to_string()),
			threshold_percent: Some(95),
			..Default::default()
		};
		assert!(GatewayEventResponse::list(&pool, &query).await?.is_empty());
		let bad_days = GatewayEventResponse::list(&pool, &GatewayEventQuery { days: Some(0), ..Default::default() }).await;
		assert!(matches!(bad_days, Err(GatewayAdminError::InvalidQuery)));
		let bad_kind = GatewayEventResponse::list(&pool, &GatewayEventQuery { kind: Some("x".to_string()), ..Default::default() }).await;
		assert!(matches!(bad_kind, Err(GatewayAdminError::InvalidQuery)));
		Ok(())
	}
}

#[cfg(test)]
mod permissions {
	use super::support::*;
	use crate::types::consts::*;
	use crate::types::*;
	use sqlx::PgPool;

	async fn user_with_role(pool: &PgPool, role: Option<&str>) -> TestResult<User> {
		let id = create_user(pool).await?;
		if let Some(role) = role {
			sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = $2").bind(id).bind(role).execute(pool).await?;
		}
		Ok(User::find_by_id(pool, &id).await?.ok_or("user")?)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_role_holds_every_developer_permission_and_users_none(pool: PgPool) -> TestResult {
		let admin = user_with_role(&pool, Some("admin")).await?;
		let member = user_with_role(&pool, Some("user")).await?;
		let plain = user_with_role(&pool, None).await?;
		for permission in [ADMIN_DEVELOPER_VIEW, ADMIN_DEVELOPER_KEYS_EDIT, ADMIN_DEVELOPER_LIMITS_EDIT, ADMIN_DEVELOPER_ACCESS_EDIT, ADMIN_DEVELOPER_AUDIT_VIEW] {
			assert!(admin.has_permission(&pool, permission).await, "{permission}");
			assert!(!member.has_permission(&pool, permission).await, "{permission}");
			assert!(!plain.has_permission(&pool, permission).await, "{permission}");
		}
		let seeded: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE name LIKE 'admin.developer.%'").fetch_one(&pool).await?;
		assert_eq!(seeded, 6);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn granular_permission_does_not_grant_others(pool: PgPool) -> TestResult {
		let user = user_with_role(&pool, None).await?;
		sqlx::query("INSERT INTO roles (name) VALUES ('gateway_viewer')").execute(&pool).await?;
		sqlx::query("INSERT INTO role_permissions (role_id, permission_id) SELECT r.id, p.id FROM roles r, permissions p WHERE r.name = 'gateway_viewer' AND p.name = 'admin.developer.view'").execute(&pool).await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = 'gateway_viewer'").bind(user.id).execute(&pool).await?;
		assert!(user.has_permission(&pool, ADMIN_DEVELOPER_VIEW).await);
		assert!(!user.has_permission(&pool, ADMIN_DEVELOPER_KEYS_EDIT).await);
		assert!(!user.has_permission(&pool, ADMIN_DEVELOPER_AUDIT_VIEW).await);
		Ok(())
	}
}
