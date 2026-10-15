#[cfg(test)]
mod support {
	use crate::tests::gateway::support::*;
	use crate::types::*;
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use uuid::Uuid;

	pub use crate::tests::gateway::support::{TestResult, create_key, create_team};

	pub fn check(assignment: u128, amount: i64, spent: i64, action: BudgetAction) -> BudgetCheck {
		BudgetCheck {
			scope: BudgetScope {
				assignment_id: Uuid::from_u128(assignment),
				user_id: None,
			},
			amount: Decimal::from(amount),
			spent: Decimal::from(spent),
			action,
		}
	}

	pub struct Fixture {
		pub user_id: Uuid,
		pub team_id: Uuid,
		pub provider_id: Uuid,
		pub model_id: Uuid,
		pub key_id: Uuid,
		pub token: String,
	}

	pub async fn fixture(pool: &PgPool, price: f64) -> TestResult<Fixture> {
		let user_id = create_user(pool).await?;
		let team_id = create_team(pool, user_id, "Budget Team", true).await?;
		let (provider_id, model_id) = create_provider_model(pool, "Budget Provider", "paid").await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(model_id)
			.bind(serde_json::json!({ "input": price, "output": price }))
			.execute(pool)
			.await?;
		let (key_id, token) = create_key(pool, user_id, team_id, serde_json::json!(["inference:write"])).await?;
		Ok(Fixture {
			user_id,
			team_id,
			provider_id,
			model_id,
			key_id,
			token,
		})
	}

	pub async fn context(pool: &PgPool, token: &str) -> TestResult<GatewayAuthContext> {
		Ok(GatewayCredential::authenticate(pool, token).await?)
	}

	pub async fn create_budget(pool: &PgPool, amount: &str, on_exceed: &str) -> TestResult<Uuid> {
		Ok(sqlx::query_scalar(
			"INSERT INTO budgets (name, amount, kind, interval, on_exceed) VALUES ($1, $2::numeric, 'pooled', 'total', $3::budget_on_exceed) RETURNING id",
		)
		.bind(format!("Budget {}", Uuid::new_v4()))
		.bind(amount)
		.bind(on_exceed)
		.fetch_one(pool)
		.await?)
	}

	pub async fn assign(pool: &PgPool, budget_id: Uuid, user_id: Option<Uuid>, team_id: Option<Uuid>, key_id: Option<Uuid>) -> TestResult<Uuid> {
		Ok(
			sqlx::query_scalar("INSERT INTO budget_assignments (budget_id, user_id, team_id, api_key_id) VALUES ($1, $2, $3, $4) RETURNING id")
				.bind(budget_id)
				.bind(user_id)
				.bind(team_id)
				.bind(key_id)
				.fetch_one(pool)
				.await?,
		)
	}

	pub async fn spend(pool: &PgPool, fixture: &Fixture, key_id: Option<Uuid>, cost: i64) -> TestResult {
		sqlx::query("INSERT INTO usage_events (user_id, team_id, api_key_id, model_id, provider_id, request_type, cost_total) VALUES ($1, $2, $3, $4, $5, 'chat', $6)")
			.bind(fixture.user_id)
			.bind(fixture.team_id)
			.bind(key_id)
			.bind(fixture.model_id)
			.bind(fixture.provider_id)
			.bind(Decimal::from(cost))
			.execute(pool)
			.await?;
		Ok(())
	}

	pub fn tokens(output: u64) -> TokenEstimate {
		TokenEstimate { input: 0, output }
	}

	pub async fn authorize(pool: &PgPool, ledger: &BudgetLedger, context: &GatewayAuthContext, estimate: TokenEstimate) -> Result<GatewayInference, GatewayModelAccessError> {
		let inference = GatewayModel::resolve_inference(pool, context, "budget provider/paid").await?;
		GatewayModel::authorize_inference(pool, ledger, context, inference, estimate).await
	}
}

#[cfg(test)]
mod ledger {
	use super::support::*;
	use crate::types::*;
	use rust_decimal::Decimal;
	use std::time::Duration;

	fn reservation_id(admission: &BudgetAdmission) -> TestResult<uuid::Uuid> {
		Ok(admission.reservation.as_ref().map(|reservation| reservation.id).ok_or("missing reservation")?)
	}

	#[tokio::test]
	async fn concurrent_admissions_near_a_hard_limit_overshoot_by_at_most_one_estimate() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let handles = (0..32)
			.map(|_| {
				let ledger = ledger.clone();
				tokio::spawn(async move { ledger.admit_checks(vec![check(1, 10, 0, BudgetAction::Block)], Decimal::from(3)).await })
			})
			.collect::<Vec<_>>();
		let mut admitted = Vec::new();
		for handle in handles {
			if let Ok(admission) = handle.await? {
				admitted.push(admission);
			}
		}
		let reserved = Decimal::from(admitted.len() as u64 * 3);
		assert_eq!(admitted.len(), 4);
		assert!(reserved < Decimal::from(10 + 3));
		Ok(())
	}

	#[tokio::test]
	async fn unfinished_reservation_is_refunded_on_drop() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let blocking = || vec![check(1, 5, 0, BudgetAction::Block)];
		let first = ledger.admit_checks(blocking(), Decimal::from(5)).await?;
		assert!(matches!(ledger.admit_checks(blocking(), Decimal::from(5)).await, Err(BudgetError::Exceeded)));
		drop(first);
		let second = ledger.admit_checks(blocking(), Decimal::from(5)).await?;
		assert!(second.reservation.is_some());
		Ok(())
	}

	#[tokio::test]
	async fn reconcile_replaces_the_estimate_once_and_settle_releases_it() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let blocking = || vec![check(1, 10, 0, BudgetAction::Block)];
		let first = ledger.admit_checks(blocking(), Decimal::from(9)).await?;
		let id = reservation_id(&first)?;
		ledger.reconcile(id, Decimal::from(1));
		ledger.reconcile(id, Decimal::from(9));
		let second = ledger.admit_checks(blocking(), Decimal::from(1)).await?;
		let third = ledger.admit_checks(blocking(), Decimal::from(8)).await?;
		assert!(matches!(ledger.admit_checks(blocking(), Decimal::from(1)).await, Err(BudgetError::Exceeded)));
		drop((second, third));
		ledger.settle(id);
		drop(first);
		let counts = ledger.state_counts().await?;
		assert_eq!(counts, BudgetStateCounts { scopes: 0, reservations: 0 });
		Ok(())
	}

	#[tokio::test]
	async fn reported_usage_survives_guard_drop_until_settled() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let admission = ledger.admit_checks(vec![check(1, 5, 0, BudgetAction::Block)], Decimal::from(5)).await?;
		let id = reservation_id(&admission)?;
		ledger.finalize(id);
		drop(admission);
		assert!(matches!(ledger.admit_checks(vec![check(1, 5, 0, BudgetAction::Block)], Decimal::from(1)).await, Err(BudgetError::Exceeded)));
		ledger.settle(id);
		assert!(ledger.admit_checks(vec![check(1, 5, 0, BudgetAction::Block)], Decimal::from(1)).await.is_ok());
		Ok(())
	}

	#[tokio::test]
	async fn warning_budgets_warn_and_allow_budgets_are_ignored() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let checks = vec![check(1, 5, 5, BudgetAction::Warn), check(2, 5, 5, BudgetAction::Allow)];
		let admission = ledger.admit_checks(checks, Decimal::from(1)).await?;
		assert!(admission.warn);
		let quiet = ledger.admit_checks(vec![check(3, 5, 0, BudgetAction::Warn)], Decimal::from(1)).await?;
		assert!(!quiet.warn);
		Ok(())
	}

	#[tokio::test]
	async fn usage_settled_after_the_spend_was_read_is_still_counted() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let first = ledger.admit_checks(vec![check(1, 6, 0, BudgetAction::Block)], Decimal::from(3)).await?;
		let id = reservation_id(&first)?;
		let epoch = ledger.epoch();
		ledger.settle(id);
		let stale = || vec![check(1, 6, 0, BudgetAction::Block)];
		let second = ledger.admit_at(stale(), Decimal::from(3), epoch).await?;
		assert!(matches!(ledger.admit_at(stale(), Decimal::from(3), epoch).await, Err(BudgetError::Exceeded)));
		let fresh = ledger.epoch();
		assert!(matches!(ledger.admit_at(vec![check(1, 6, 3, BudgetAction::Block)], Decimal::from(1), fresh).await, Err(BudgetError::Exceeded)));
		drop((first, second));
		Ok(())
	}

	#[tokio::test]
	async fn stopped_ledger_fails_closed() {
		let ledger = BudgetLedger::unavailable();
		assert!(matches!(ledger.admit_checks(Vec::new(), Decimal::ZERO).await, Err(BudgetError::Unavailable)));
	}

	#[tokio::test(start_paused = true)]
	async fn unsettled_reported_usage_expires() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let admission = ledger.admit_checks(vec![check(1, 5, 0, BudgetAction::Block)], Decimal::from(5)).await?;
		ledger.finalize(reservation_id(&admission)?);
		drop(admission);
		tokio::time::sleep(Duration::from_secs(400)).await;
		assert_eq!(ledger.state_counts().await?, BudgetStateCounts { scopes: 0, reservations: 0 });
		Ok(())
	}
}

#[cfg(test)]
mod scope {
	use super::support::*;
	use crate::types::*;
	use ::axum::response::Response;
	use rust_decimal::Decimal;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn exhausted_user_budget_blocks_gateway_requests(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "10", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		assert!(authorize(&pool, &ledger, &context, tokens(1)).await.is_ok());
		spend(&pool, &fixture, Some(fixture.key_id), 10).await?;
		assert!(matches!(authorize(&pool, &ledger, &context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_of_another_team_blocks_a_key_pinned_elsewhere(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let other_team = create_team(&pool, fixture.user_id, "Other Budget Team", false).await?;
		let budget = create_budget(&pool, "5", "block").await?;
		assign(&pool, budget, None, Some(other_team), None).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		assert!(authorize(&pool, &ledger, &context, tokens(1)).await.is_ok());
		spend(&pool, &fixture, None, 5).await?;
		assert!(matches!(authorize(&pool, &ledger, &context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_budget_blocks_only_its_own_key(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let (other_key, other_token) = create_key(&pool, fixture.user_id, fixture.team_id, serde_json::json!(["inference:write"])).await?;
		let budget = create_budget(&pool, "5", "block").await?;
		assign(&pool, budget, None, None, Some(fixture.key_id)).await?;
		spend(&pool, &fixture, Some(fixture.key_id), 5).await?;
		spend(&pool, &fixture, Some(other_key), 100).await?;
		let ledger = BudgetLedger::spawn();
		let blocked = context(&pool, &fixture.token).await?;
		let allowed = context(&pool, &other_token).await?;
		assert!(matches!(authorize(&pool, &ledger, &blocked, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		assert!(authorize(&pool, &ledger, &allowed, tokens(1)).await.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn warning_budget_admits_and_adds_the_warning_header(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "5", "warn").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		let quiet = authorize(&pool, &ledger, &context, tokens(1)).await?;
		assert!(quiet.finish(Response::new(::axum::body::Body::empty())).headers().get(BUDGET_WARNING_HEADER).is_none());
		spend(&pool, &fixture, None, 5).await?;
		let warned = authorize(&pool, &ledger, &context, tokens(1)).await?;
		assert!(warned.finish(Response::new(::axum::body::Body::empty())).headers().contains_key(BUDGET_WARNING_HEADER));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn most_restrictive_block_wins_over_a_warning(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let warn = create_budget(&pool, "100", "warn").await?;
		let block = create_budget(&pool, "5", "block").await?;
		assign(&pool, warn, Some(fixture.user_id), None, None).await?;
		assign(&pool, block, None, None, Some(fixture.key_id)).await?;
		spend(&pool, &fixture, Some(fixture.key_id), 5).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		assert!(matches!(authorize(&pool, &ledger, &context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn free_models_bypass_exhausted_budgets(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 0.0).await?;
		let budget = create_budget(&pool, "5", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		spend(&pool, &fixture, None, 5).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		let inference = authorize(&pool, &ledger, &context, tokens(1000)).await?;
		assert!(inference.budget.reservation.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn gateway_and_web_chat_share_spend_and_reservations(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "10", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		spend(&pool, &fixture, Some(fixture.key_id), 4).await?;
		spend(&pool, &fixture, None, 3).await?;
		let status = Budget::status_for(&pool, BudgetSubject::user(fixture.user_id)).await?;
		assert_eq!(status.budgets.len(), 1);
		assert_eq!(status.budgets[0].spent, Decimal::from(7));
		let ledger = BudgetLedger::spawn();
		let web_chat = ledger.reserve_for_model(&pool, BudgetSubject::user(fixture.user_id), &fixture.model_id, tokens(3)).await?;
		let context = context(&pool, &fixture.token).await?;
		assert!(matches!(authorize(&pool, &ledger, &context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		drop(web_chat);
		assert!(authorize(&pool, &ledger, &context, tokens(1)).await.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn concurrent_gateway_requests_near_the_limit_overshoot_by_at_most_one_estimate(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "10", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		spend(&pool, &fixture, None, 2).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		let handles = (0..16)
			.map(|_| {
				let (pool, ledger, context) = (pool.clone(), ledger.clone(), context.clone());
				tokio::spawn(async move { authorize(&pool, &ledger, &context, tokens(3)).await.ok() })
			})
			.collect::<Vec<_>>();
		let mut admitted = Vec::new();
		for handle in handles {
			if let Some(inference) = handle.await? {
				admitted.push(inference);
			}
		}
		assert_eq!(admitted.len(), 3);
		assert!(Decimal::from(2 + admitted.len() as u64 * 3) < Decimal::from(10 + 3));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn requests_settling_during_admission_never_exceed_the_limit_by_more_than_one_estimate(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "10", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		let ledger = BudgetLedger::spawn();
		let subject = BudgetSubject::user(fixture.user_id);
		let handles = (0..24)
			.map(|_| {
				let (pool, ledger) = (pool.clone(), ledger.clone());
				let (user_id, team_id, provider_id, model_id) = (fixture.user_id, fixture.team_id, fixture.provider_id, fixture.model_id);
				tokio::spawn(async move {
					let request = BudgetRequest { subject, estimated_cost: Decimal::from(3) };
					let admission = ledger.reserve(&pool, request).await.ok()?;
					let id = admission.reservation.as_ref()?.id;
					ledger.finalize(id);
					sqlx::query("INSERT INTO usage_events (user_id, team_id, model_id, provider_id, request_type, cost_total) VALUES ($1, $2, $3, $4, 'chat', 3)")
						.bind(user_id)
						.bind(team_id)
						.bind(model_id)
						.bind(provider_id)
						.execute(&pool)
						.await
						.ok()?;
					ledger.reconcile(id, Decimal::from(3));
					ledger.settle(id);
					Some(admission)
				})
			})
			.collect::<Vec<_>>();
		let mut admitted = 0_u64;
		for handle in handles {
			admitted += u64::from(handle.await?.is_some());
		}
		let stored: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(cost_total), 0) FROM usage_events").fetch_one(&pool).await?;
		assert_eq!(stored, Decimal::from(admitted * 3));
		assert!(stored < Decimal::from(10 + 3));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn reset_keeps_usage_history_and_reopens_the_budget(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "5", "block").await?;
		let assignment = assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		spend(&pool, &fixture, None, 5).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, &fixture.token).await?;
		assert!(authorize(&pool, &ledger, &context, tokens(1)).await.is_err());
		sqlx::query("INSERT INTO budget_reset_events (assignment_id) VALUES ($1)").bind(assignment).execute(&pool).await?;
		assert!(authorize(&pool, &ledger, &context, tokens(1)).await.is_ok());
		let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_events WHERE user_id = $1").bind(fixture.user_id).fetch_one(&pool).await?;
		assert_eq!(events, 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn stopped_ledger_rejects_paid_requests(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let context = context(&pool, &fixture.token).await?;
		let result = authorize(&pool, &BudgetLedger::unavailable(), &context, tokens(1)).await;
		assert!(matches!(result, Err(GatewayModelAccessError::BudgetUnavailable)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_errors_render_as_openai_errors(pool: PgPool) -> TestResult {
		crate::config::Config::init(&pool).await;
		crate::i18n::I18n::init(&pool).await;
		let response = crate::utils::openai_gateway::model_access_error_response(GatewayModelAccessError::BudgetExceeded, "budget provider/paid");
		assert_eq!(response.status().as_u16(), 429);
		let body: serde_json::Value = serde_json::from_slice(&::axum::body::to_bytes(response.into_body(), 10000).await?)?;
		assert_eq!(body["error"]["type"], "insufficient_quota");
		assert_eq!(body["error"]["code"], "budget_exceeded");
		let response = crate::utils::openai_gateway::model_access_error_response(GatewayModelAccessError::BudgetUnavailable, "budget provider/paid");
		assert_eq!(response.status().as_u16(), 503);
		Ok(())
	}
}

#[cfg(test)]
mod keys {
	use super::support::*;
	use crate::types::*;
	use rust_decimal::Decimal;
	use sqlx::PgPool;

	fn budget_request(amount: i64, interval: &str, on_exceed: &str) -> SetGatewayKeyBudgetRequest {
		SetGatewayKeyBudgetRequest {
			amount: Decimal::from(amount),
			interval: interval.to_owned(),
			on_exceed: on_exceed.to_owned(),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn key_budget_is_set_replaced_audited_and_cleared(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		assert!(GatewayApiKey::budget(&pool, &fixture.user_id, &fixture.key_id).await?.is_none());
		let created = GatewayApiKey::replace_budget(&pool, &fixture.user_id, &fixture.key_id, &budget_request(5, "monthly", "block")).await?;
		assert_eq!((created.amount, created.interval.as_str(), created.on_exceed.as_str()), (Decimal::from(5), "monthly", "block"));
		let replaced = GatewayApiKey::replace_budget(&pool, &fixture.user_id, &fixture.key_id, &budget_request(7, "daily", "warn")).await?;
		assert_eq!((replaced.amount, replaced.interval.as_str()), (Decimal::from(7), "daily"));
		let budgets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM budgets").fetch_one(&pool).await?;
		assert_eq!(budgets, 1);
		assert_eq!(Budget::list(&pool, 1, 50, None).await?.items.len(), 0);
		GatewayApiKey::clear_budget(&pool, &fixture.user_id, &fixture.key_id).await?;
		assert!(GatewayApiKey::budget(&pool, &fixture.user_id, &fixture.key_id).await?.is_none());
		let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM budgets").fetch_one(&pool).await?;
		assert_eq!(remaining, 0);
		let events: Vec<String> = sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_id = $1 AND event LIKE 'gateway_api_key_budget_%'")
			.bind(fixture.key_id)
			.fetch_all(&pool)
			.await?;
		assert_eq!(events.len(), 3);
		assert!(events.iter().any(|event| event == "gateway_api_key_budget_cleared"));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_key_budgets_are_rejected(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		for request in [budget_request(0, "daily", "block"), budget_request(-1, "daily", "block"), budget_request(5, "yearly", "block"), budget_request(5, "daily", "allow")] {
			let result = GatewayApiKey::replace_budget(&pool, &fixture.user_id, &fixture.key_id, &request).await;
			assert!(matches!(result, Err(GatewayKeyError::InvalidBudget)));
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn only_the_owner_can_manage_a_key_budget(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let stranger = uuid::Uuid::new_v4();
		let result = GatewayApiKey::replace_budget(&pool, &stranger, &fixture.key_id, &budget_request(5, "daily", "block")).await;
		assert!(matches!(result, Err(GatewayKeyError::NotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn rotation_keeps_one_shared_allowance_and_past_spend(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		GatewayApiKey::replace_budget(&pool, &fixture.user_id, &fixture.key_id, &budget_request(5, "total", "block")).await?;
		spend(&pool, &fixture, Some(fixture.key_id), 5).await?;
		let (replacement, _, token) = GatewayApiKey::rotate(&pool, &fixture.user_id, &fixture.key_id, Some(60)).await?;
		let shared: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM budgets").fetch_one(&pool).await?;
		assert_eq!(shared, 1);
		let copied = GatewayApiKey::budget(&pool, &fixture.user_id, &replacement.id).await?.ok_or("missing shared budget")?;
		assert_eq!((copied.amount, copied.spent, copied.exhausted), (Decimal::from(5), Decimal::from(5), true));
		let ledger = BudgetLedger::spawn();
		let new_context = context(&pool, &token).await?;
		assert!(matches!(authorize(&pool, &ledger, &new_context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		let old_context = context(&pool, &fixture.token).await?;
		assert!(matches!(authorize(&pool, &ledger, &old_context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		GatewayApiKey::clear_budget(&pool, &fixture.user_id, &replacement.id).await?;
		assert!(GatewayApiKey::budget(&pool, &fixture.user_id, &fixture.key_id).await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn overlapping_keys_share_in_flight_reservations(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		GatewayApiKey::replace_budget(&pool, &fixture.user_id, &fixture.key_id, &budget_request(5, "total", "block")).await?;
		let (_, _, token) = GatewayApiKey::rotate(&pool, &fixture.user_id, &fixture.key_id, Some(60)).await?;
		let ledger = BudgetLedger::spawn();
		let old_context = context(&pool, &fixture.token).await?;
		let new_context = context(&pool, &token).await?;
		let held = authorize(&pool, &ledger, &old_context, tokens(5)).await?;
		assert!(matches!(authorize(&pool, &ledger, &new_context, tokens(1)).await, Err(GatewayModelAccessError::BudgetExceeded)));
		drop(held);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_assignments_need_exactly_one_target(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool, 1_000_000.0).await?;
		let budget = create_budget(&pool, "5", "block").await?;
		assert!(assign(&pool, budget, Some(fixture.user_id), None, Some(fixture.key_id)).await.is_err());
		assert!(assign(&pool, budget, None, None, None).await.is_err());
		assert!(assign(&pool, budget, None, None, Some(fixture.key_id)).await.is_ok());
		Ok(())
	}
}
