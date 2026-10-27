#[cfg(test)]
mod support {
	use crate::tests::gateway::support::{create_provider_model, insert_key};
	use crate::types::*;
	use sqlx::PgPool;
	use uuid::Uuid;

	pub use crate::tests::gateway::support::TestResult;

	pub struct Setup {
		pub admin_id: Uuid,
		pub member_id: Uuid,
		pub outsider_id: Uuid,
		pub team_id: Uuid,
		pub model_id: Uuid,
		pub provider_id: Uuid,
	}

	pub async fn user(pool: &PgPool, name: &str) -> TestResult<Uuid> {
		Ok(
			sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ($1, $2, 'hash') RETURNING id")
				.bind(format!("{name}@example.com"))
				.bind(name)
				.fetch_one(pool)
				.await?,
		)
	}

	pub async fn setup(pool: &PgPool) -> TestResult<Setup> {
		let admin_id = user(pool, "teamadmin").await?;
		let member_id = user(pool, "teammember").await?;
		let outsider_id = user(pool, "outsider").await?;
		let team_id: Uuid = sqlx::query_scalar("INSERT INTO teams (name, allow_all_models) VALUES ('Service Team', true) RETURNING id")
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'admin'), ($1, $3, 'member')")
			.bind(team_id)
			.bind(admin_id)
			.bind(member_id)
			.execute(pool)
			.await?;
		let (provider_id, model_id) = create_provider_model(pool, "Budget Provider", "paid").await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(model_id)
			.bind(serde_json::json!({ "input": 1_000_000.0, "output": 1_000_000.0 }))
			.execute(pool)
			.await?;
		Ok(Setup {
			admin_id,
			member_id,
			outsider_id,
			team_id,
			model_id,
			provider_id,
		})
	}

	pub fn request(team_id: Uuid, service_account: bool) -> CreateGatewayKeyRequest {
		CreateGatewayKeyRequest {
			name: "Service".to_string(),
			team_id,
			scopes: vec!["inference:write".to_string()],
			expires_at: None,
			max_concurrent_requests: None,
			service_account,
		}
	}

	pub async fn service_key(pool: &PgPool, setup: &Setup) -> TestResult<(GatewayApiKey, String)> {
		Ok(GatewayApiKey::create(pool, &setup.admin_id, &request(setup.team_id, true)).await?)
	}

	pub async fn events_for(pool: &PgPool, target_id: Uuid) -> TestResult<Vec<String>> {
		Ok(sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_id = $1 ORDER BY xmin::text::bigint, id").bind(target_id).fetch_all(pool).await?)
	}

	pub async fn personal_key(pool: &PgPool, user_id: Uuid, team_id: Uuid) -> TestResult<Uuid> {
		Ok(insert_key(pool, Some(user_id), team_id, serde_json::json!(["inference:write"])).await?.0)
	}
}

#[cfg(test)]
mod roles {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn members_default_to_member_and_admin_is_detected(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		assert!(Team::is_admin(&pool, &setup.team_id, &setup.admin_id).await?);
		assert!(!Team::is_admin(&pool, &setup.team_id, &setup.member_id).await?);
		assert!(!Team::is_admin(&pool, &setup.team_id, &setup.outsider_id).await?);
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(setup.team_id).bind(setup.outsider_id).execute(&pool).await?;
		let role: String = sqlx::query_scalar("SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2").bind(setup.team_id).bind(setup.outsider_id).fetch_one(&pool).await?;
		assert_eq!(role, "member");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_roles_are_rejected_by_the_database(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let result = sqlx::query("UPDATE team_members SET role = 'owner' WHERE team_id = $1 AND user_id = $2").bind(setup.team_id).bind(setup.member_id).execute(&pool).await;
		assert!(result.is_err());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn setting_a_role_is_audited_and_unknown_members_are_reported(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let team = Team::find_by_id(&pool, &setup.team_id).await?.ok_or("missing team")?;
		assert!(team.set_member_role(&pool, &setup.admin_id, &setup.member_id, TeamRole::Admin).await?);
		assert!(Team::is_admin(&pool, &setup.team_id, &setup.member_id).await?);
		assert!(!team.set_member_role(&pool, &setup.admin_id, &setup.outsider_id, TeamRole::Admin).await?);
		assert!(!Team::is_admin(&pool, &setup.team_id, &setup.outsider_id).await?);
		let events = events_for(&pool, setup.team_id).await?;
		assert_eq!(events, vec!["team_member_role_updated".to_string()]);
		let members = team.members(&pool).await?;
		assert_eq!(members.iter().filter(|member| member.role == "admin").count(), 2);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn replacing_members_keeps_roles_of_remaining_members(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let team = Team::find_by_id(&pool, &setup.team_id).await?.ok_or("missing team")?;
		team.set_members(&pool, &[setup.admin_id, setup.outsider_id]).await?;
		assert!(Team::is_admin(&pool, &setup.team_id, &setup.admin_id).await?);
		let members = team.members(&pool).await?;
		assert_eq!(members.len(), 2);
		assert!(members.iter().all(|member| member.id != setup.member_id));
		Team::set_user_teams(&pool, &setup.admin_id, &[setup.team_id]).await?;
		assert!(Team::is_admin(&pool, &setup.team_id, &setup.admin_id).await?);
		Ok(())
	}
}

#[cfg(test)]
mod management {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn only_team_and_installation_admins_create_service_keys(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let denied = GatewayApiKey::create(&pool, &setup.member_id, &request(setup.team_id, true)).await;
		assert!(matches!(denied, Err(GatewayKeyError::ServiceAccountForbidden)));
		let denied = GatewayApiKey::create(&pool, &setup.outsider_id, &request(setup.team_id, true)).await;
		assert!(matches!(denied, Err(GatewayKeyError::ServiceAccountForbidden)));

		let (key, token) = service_key(&pool, &setup).await?;
		assert!(key.user_id.is_none());
		assert!(token.starts_with("oxc_"));
		let created_by: Option<uuid::Uuid> = sqlx::query_scalar("SELECT created_by FROM gateway_api_keys WHERE id = $1").bind(key.id).fetch_one(&pool).await?;
		assert_eq!(created_by, Some(setup.admin_id));
		assert_eq!(events_for(&pool, key.id).await?, vec!["gateway_api_key_created".to_string()]);

		let installation_admin = KeyActor::new(&setup.outsider_id, true);
		let (second, _) = GatewayApiKey::create(&pool, installation_admin, &request(setup.team_id, true)).await?;
		assert!(second.user_id.is_none());
		let missing_team = GatewayApiKey::create(&pool, installation_admin, &request(uuid::Uuid::new_v4(), true)).await;
		assert!(matches!(missing_team, Err(GatewayKeyError::ServiceAccountForbidden)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn members_cannot_manage_service_keys(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, _) = service_key(&pool, &setup).await?;
		let update = UpdateGatewayKeyRequest {
			name: Some("Renamed".to_string()),
			..UpdateGatewayKeyRequest::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &setup.member_id, &key.id, &update).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::revoke(&pool, &setup.member_id, &key.id).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::rotate(&pool, &setup.member_id, &key.id, None).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::list_rules(&pool, &setup.member_id, &key.id).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::revoke(&pool, &setup.outsider_id, &key.id).await, Err(GatewayKeyError::NotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_admins_rotate_and_revoke_service_keys_with_audit(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, old_token) = service_key(&pool, &setup).await?;
		let (replacement, previous, new_token) = GatewayApiKey::rotate(&pool, &setup.admin_id, &key.id, Some(3600)).await?;
		assert!(replacement.user_id.is_none());
		assert_eq!(replacement.team_id, setup.team_id);
		assert!(previous.expires_at.is_some());
		assert_ne!(old_token, new_token);
		assert_eq!(events_for(&pool, key.id).await?, vec!["gateway_api_key_created".to_string(), "gateway_api_key_rotated".to_string()]);

		let revoked = GatewayApiKey::revoke(&pool, &setup.admin_id, &replacement.id).await?;
		assert!(revoked.revoked_at.is_some());
		assert!(matches!(GatewayCredential::authenticate(&pool, &new_token).await, Err(GatewayAuthError::Invalid)));
		assert_eq!(events_for(&pool, replacement.id).await?, vec!["gateway_api_key_revoked".to_string()]);
		let installation = KeyActor::new(&setup.outsider_id, true);
		let (other, _) = service_key(&pool, &setup).await?;
		assert!(GatewayApiKey::revoke(&pool, installation, &other.id).await?.revoked_at.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_admins_revoke_but_cannot_change_members_personal_keys(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let key_id = personal_key(&pool, setup.member_id, setup.team_id).await?;
		let update = UpdateGatewayKeyRequest {
			is_enabled: Some(false),
			..UpdateGatewayKeyRequest::default()
		};
		assert!(matches!(GatewayApiKey::update(&pool, &setup.admin_id, &key_id, &update).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::rotate(&pool, &setup.admin_id, &key_id, None).await, Err(GatewayKeyError::NotFound)));
		assert!(matches!(GatewayApiKey::revoke(&pool, &setup.outsider_id, &key_id).await, Err(GatewayKeyError::NotFound)));
		let other_member = user(&pool, "othermember").await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(setup.team_id).bind(other_member).execute(&pool).await?;
		assert!(matches!(GatewayApiKey::revoke(&pool, &other_member, &key_id).await, Err(GatewayKeyError::NotFound)));

		let revoked = GatewayApiKey::revoke(&pool, &setup.admin_id, &key_id).await?;
		assert!(revoked.revoked_at.is_some());
		assert_eq!(revoked.user_id, Some(setup.member_id));
		let revoked_by: Option<uuid::Uuid> = sqlx::query_scalar("SELECT revoked_by FROM gateway_api_keys WHERE id = $1").bind(key_id).fetch_one(&pool).await?;
		assert_eq!(revoked_by, Some(setup.admin_id));
		assert_eq!(events_for(&pool, key_id).await?, vec!["gateway_api_key_revoked".to_string()]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_listing_shows_all_team_keys_to_admins_only(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (service, _) = service_key(&pool, &setup).await?;
		let personal = personal_key(&pool, setup.member_id, setup.team_id).await?;
		let admin_view = GatewayTeamKeysResponse::load(&pool, &setup.admin_id).await?;
		assert_eq!(admin_view.teams.len(), 1);
		let ids: Vec<_> = admin_view.keys.iter().map(|key| key.key.id).collect();
		assert!(ids.contains(&service.id) && ids.contains(&personal));
		let personal_row = admin_view.keys.iter().find(|key| key.key.id == personal).ok_or("missing personal key")?;
		assert_eq!(personal_row.owner_username.as_deref(), Some("teammember"));
		let member_view = GatewayTeamKeysResponse::load(&pool, &setup.member_id).await?;
		assert!(member_view.teams.is_empty() && member_view.keys.is_empty());
		let installation_view = GatewayTeamKeysResponse::load(&pool, KeyActor::new(&setup.outsider_id, true)).await?;
		assert_eq!(installation_view.keys.len(), 1);
		assert_eq!(installation_view.keys[0].key.id, service.id);
		assert!(GatewayApiKey::list_for_user(&pool, &setup.admin_id).await?.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod authentication {
	use super::support::*;
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn service_key_works_after_its_creator_leaves_or_is_deleted(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, token) = service_key(&pool, &setup).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert_eq!(context.user_id, None);
		assert_eq!(context.team_id, setup.team_id);
		assert_eq!(context.key_id, key.id);
		assert!(context.allows("inference:write"));

		sqlx::query("DELETE FROM team_members WHERE user_id = $1").bind(setup.admin_id).execute(&pool).await?;
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		sqlx::query("DELETE FROM users WHERE id = $1").bind(setup.admin_id).execute(&pool).await?;
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn service_key_still_respects_team_and_key_state(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, token) = service_key(&pool, &setup).await?;
		sqlx::query("UPDATE teams SET gateway_enabled = false WHERE id = $1").bind(setup.team_id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		sqlx::query("UPDATE teams SET gateway_enabled = true WHERE id = $1").bind(setup.team_id).execute(&pool).await?;
		sqlx::query("UPDATE gateway_api_keys SET admin_disabled_at = NOW() WHERE id = $1").bind(key.id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		sqlx::query("UPDATE gateway_api_keys SET admin_disabled_at = NULL WHERE id = $1").bind(key.id).execute(&pool).await?;
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		sqlx::query("DELETE FROM teams WHERE id = $1").bind(setup.team_id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn personal_keys_still_need_membership(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (_, token) = crate::tests::gateway::support::insert_key(&pool, Some(setup.member_id), setup.team_id, serde_json::json!(["inference:read"])).await?;
		assert!(GatewayCredential::authenticate(&pool, &token).await.is_ok());
		sqlx::query("DELETE FROM team_members WHERE user_id = $1").bind(setup.member_id).execute(&pool).await?;
		assert!(matches!(GatewayCredential::authenticate(&pool, &token).await, Err(GatewayAuthError::Invalid)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn service_key_lists_and_resolves_team_models_without_membership(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (_, token) = service_key(&pool, &setup).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let models = GatewayModel::list_for_context(&pool, &context).await?;
		assert_eq!(models.len(), 1);
		assert!(GatewayModel::resolve_inference(&pool, &context, "budget provider/paid").await.is_ok());
		sqlx::query("UPDATE teams SET allow_all_models = false WHERE id = $1").bind(setup.team_id).execute(&pool).await?;
		assert!(matches!(
			GatewayModel::resolve_inference(&pool, &context, "budget provider/paid").await,
			Err(GatewayModelAccessError::NotFound)
		));
		Ok(())
	}
}

#[cfg(test)]
mod budgets {
	use super::support::*;
	use crate::types::*;
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	#[derive(Default)]
	struct Target {
		team_id: Option<Uuid>,
		user_id: Option<Uuid>,
		key_id: Option<Uuid>,
	}

	async fn budget(pool: &PgPool, amount: &str, kind: &str, target: Target) -> TestResult {
		let budget_id: Uuid = sqlx::query_scalar("INSERT INTO budgets (name, amount, kind, interval, on_exceed) VALUES ($1, $2::numeric, $3::budget_kind, 'total', 'block') RETURNING id")
			.bind(format!("Budget {}", Uuid::new_v4()))
			.bind(amount)
			.bind(kind)
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id, team_id, api_key_id) VALUES ($1, $2, $3, $4)")
			.bind(budget_id)
			.bind(target.user_id)
			.bind(target.team_id)
			.bind(target.key_id)
			.execute(pool)
			.await?;
		Ok(())
	}

	async fn spend(pool: &PgPool, setup: &Setup, user_id: Option<Uuid>, key_id: Option<Uuid>, cost: i64) -> TestResult {
		sqlx::query("INSERT INTO usage_events (user_id, team_id, api_key_id, model_id, provider_id, request_type, cost_total) VALUES ($1, $2, $3, $4, $5, 'chat', $6)")
			.bind(user_id)
			.bind(setup.team_id)
			.bind(key_id)
			.bind(setup.model_id)
			.bind(setup.provider_id)
			.bind(Decimal::from(cost))
			.execute(pool)
			.await?;
		Ok(())
	}

	async fn authorize(pool: &PgPool, ledger: &BudgetLedger, context: &GatewayAuthContext, output: u64) -> Result<GatewayInference, GatewayModelAccessError> {
		let inference = GatewayModel::resolve_inference(pool, context, "budget provider/paid").await?;
		GatewayModel::authorize_inference(pool, ledger, context, inference, TokenEstimate { input: 0, output }).await
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn service_keys_count_against_team_budgets(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, token) = service_key(&pool, &setup).await?;
		budget(&pool, "10", "pooled", Target { team_id: Some(setup.team_id), ..Target::default() }).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let ledger = BudgetLedger::spawn();
		assert!(authorize(&pool, &ledger, &context, 5).await.is_ok());
		spend(&pool, &setup, None, Some(key.id), 10).await?;
		let status = Budget::status_for(&pool, context.budget_subject()).await?;
		assert_eq!(status.budgets.len(), 1);
		assert_eq!(status.budgets[0].spent, Decimal::from(10));
		assert!(matches!(authorize(&pool, &ledger, &context, 5).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_spend_by_members_and_service_keys_is_shared(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (_, token) = service_key(&pool, &setup).await?;
		budget(&pool, "10", "pooled", Target { team_id: Some(setup.team_id), ..Target::default() }).await?;
		spend(&pool, &setup, Some(setup.member_id), None, 10).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let ledger = BudgetLedger::spawn();
		assert!(matches!(authorize(&pool, &ledger, &context, 1).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn user_budgets_never_apply_to_service_keys(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (_, token) = service_key(&pool, &setup).await?;
		budget(&pool, "1", "pooled", Target { user_id: Some(setup.admin_id), ..Target::default() }).await?;
		budget(&pool, "1", "per_user", Target { team_id: Some(setup.team_id), ..Target::default() }).await?;
		spend(&pool, &setup, Some(setup.admin_id), None, 5).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		assert!(Budget::status_for(&pool, context.budget_subject()).await?.budgets.is_empty());
		let ledger = BudgetLedger::spawn();
		assert!(authorize(&pool, &ledger, &context, 5).await.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn service_keys_respect_their_own_budget_and_other_teams_do_not_apply(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, token) = service_key(&pool, &setup).await?;
		let other_team: Uuid = sqlx::query_scalar("INSERT INTO teams (name) VALUES ('Other Team') RETURNING id").fetch_one(&pool).await?;
		budget(&pool, "1", "pooled", Target { team_id: Some(other_team), ..Target::default() }).await?;
		budget(&pool, "4", "pooled", Target { key_id: Some(key.id), ..Target::default() }).await?;
		spend(&pool, &setup, None, Some(key.id), 4).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let status = Budget::status_for(&pool, context.budget_subject()).await?;
		assert_eq!(status.budgets.len(), 1);
		let ledger = BudgetLedger::spawn();
		assert!(matches!(authorize(&pool, &ledger, &context, 1).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}
}

#[cfg(test)]
mod usage {
	use super::support::*;
	use crate::tests::gateway::support::job_state;
	use crate::types::*;
	use crate::utils::omniference_cost::OxideCostQueue;
	use omniference::catalog::UsageBreakdown;
	use omniference::middleware::cost::{CostFinalization, CostSink};
	use omniference::stream::CostDetails;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use uuid::Uuid;

	type UsageRow = (Option<Uuid>, Option<Uuid>, Option<Uuid>, i32);

	fn metadata(setup: &Setup, key_id: Option<Uuid>, with_team: bool) -> BTreeMap<String, String> {
		let mut metadata = BTreeMap::from([
			("oxide_model_id".to_string(), setup.model_id.to_string()),
			("oxide_request_type".to_string(), "gateway.chat".to_string()),
			(REQUEST_ID_METADATA_KEY.to_string(), Uuid::new_v4().to_string()),
		]);
		if let Some(key_id) = key_id {
			metadata.insert("oxide_api_key_id".to_string(), key_id.to_string());
		}
		if with_team {
			metadata.insert("oxide_team_id".to_string(), setup.team_id.to_string());
		}
		metadata
	}

	async fn record(pool: &PgPool, metadata: &BTreeMap<String, String>) {
		let queue = OxideCostQueue::spawn(job_state(pool.clone()));
		let cost = CostDetails {
			total: 0.0,
			prompt: None,
			completion: None,
			reasoning: None,
		};
		let usage = UsageBreakdown {
			input_tokens: 3,
			output_tokens: 4,
			..Default::default()
		};
		queue.record_with_context("Budget Provider", "paid", &cost, CostFinalization::Done, metadata, &usage);
		queue.shutdown().await;
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn service_usage_is_attributed_to_team_and_key_without_a_user(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, _) = service_key(&pool, &setup).await?;
		record(&pool, &metadata(&setup, Some(key.id), true)).await;
		let rows: Vec<UsageRow> = sqlx::query_as("SELECT user_id, team_id, api_key_id, input_tokens FROM usage_events").fetch_all(&pool).await?;
		assert_eq!(rows, vec![(None, Some(setup.team_id), Some(key.id), 3)]);
		let logged: Vec<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT user_id, api_key_id FROM gateway_request_logs").fetch_all(&pool).await?;
		assert_eq!(logged, vec![(None, Some(key.id))]);
		let cost: rust_decimal::Decimal = sqlx::query_scalar("SELECT cost_total FROM usage_events").fetch_one(&pool).await?;
		assert_eq!(cost, rust_decimal::Decimal::from(7));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn usage_without_user_team_or_key_is_not_recorded(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, _) = service_key(&pool, &setup).await?;
		record(&pool, &metadata(&setup, Some(key.id), false)).await;
		record(&pool, &metadata(&setup, None, true)).await;
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_events").fetch_one(&pool).await?;
		assert_eq!(count, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn user_usage_keeps_its_user_and_survives_user_deletion(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let key_id = personal_key(&pool, setup.member_id, setup.team_id).await?;
		let mut personal = metadata(&setup, Some(key_id), true);
		personal.insert("oxide_user_id".to_string(), setup.member_id.to_string());
		record(&pool, &personal).await;
		let user: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM usage_events").fetch_one(&pool).await?;
		assert_eq!(user, Some(setup.member_id));
		sqlx::query("DELETE FROM users WHERE id = $1").bind(setup.member_id).execute(&pool).await?;
		let rows: Vec<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT user_id, team_id FROM usage_events").fetch_all(&pool).await?;
		assert_eq!(rows, vec![(None, Some(setup.team_id))]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn analytics_handle_rows_without_a_user(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let (key, _) = service_key(&pool, &setup).await?;
		record(&pool, &metadata(&setup, Some(key.id), true)).await;
		let team_filter = UsageFilter {
			team_id: Some(setup.team_id),
			..UsageFilter::default()
		};
		let by_team = UsageEvent::analytics(&pool, None, None, "team", team_filter).await?;
		assert_eq!(by_team.len(), 1);
		assert_eq!(by_team[0].request_count, 1);
		let by_user = UsageEvent::analytics(&pool, None, None, "user", team_filter).await?;
		assert_eq!(by_user.len(), 1);
		assert_eq!(by_user[0].id, None);
		assert_eq!(by_user[0].request_count, 1);
		assert_eq!(by_user[0].cost_total, by_team[0].cost_total);
		let by_key = UsageEvent::api_key_analytics(&pool, None, None, team_filter).await?;
		assert_eq!(by_key.len(), 1);
		assert_eq!(by_key[0].id, Some(key.id));
		let for_user = UsageFilter {
			user_id: Some(setup.admin_id),
			..UsageFilter::default()
		};
		assert!(UsageEvent::analytics(&pool, None, None, "model", for_user).await?.is_empty());
		assert!(UsageEvent::analytics(&pool, None, None, "user", for_user).await?.is_empty());
		let mut personal = metadata(&setup, None, true);
		personal.insert("oxide_user_id".to_string(), setup.admin_id.to_string());
		record(&pool, &personal).await;
		let by_user = UsageEvent::analytics(&pool, None, None, "user", team_filter).await?;
		let by_team = UsageEvent::analytics(&pool, None, None, "team", team_filter).await?;
		assert_eq!(by_user.len(), 2);
		assert_eq!(by_user.iter().filter(|row| row.id.is_none()).count(), 1);
		assert_eq!(by_user.iter().map(|row| row.request_count).sum::<i64>(), by_team[0].request_count);
		assert_eq!(by_user.iter().map(|row| row.cost_total).sum::<rust_decimal::Decimal>(), by_team[0].cost_total);
		let only_admin = UsageEvent::analytics(&pool, None, None, "user", for_user).await?;
		assert_eq!(only_admin.len(), 1);
		assert_eq!(only_admin[0].id, Some(setup.admin_id));
		Ok(())
	}
}
