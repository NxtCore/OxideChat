#[cfg(test)]
mod guard {
	use crate::tests::gateway::support::job_state;
	use crate::tests::support::TestResult;
	use crate::types::*;
	use rust_decimal::Decimal;
	use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
	use std::time::Duration;
	use uuid::Uuid;

	async fn guard_with_reservation(ledger: &BudgetLedger) -> TestResult<ChatUsageGuard> {
		let pool = PgPoolOptions::new().acquire_timeout(Duration::from_millis(100)).connect_lazy_with(PgConnectOptions::new());
		let mut state = job_state(pool);
		std::sync::Arc::get_mut(&mut state).ok_or("state is shared")?.budget_ledger = ledger.clone();
		let admission = ledger.admit_checks(Vec::new(), Decimal::from(5)).await?;
		Ok(ChatUsageGuard::new(state, admission.reservation, Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()))
	}

	#[tokio::test]
	async fn dropping_without_consumed_tokens_refunds_the_reservation() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let guard = guard_with_reservation(&ledger).await?;
		drop(guard);
		let counts = ledger.state_counts().await?;
		assert_eq!(counts.reservations, 0);
		Ok(())
	}

	#[tokio::test]
	async fn dropping_after_consumed_tokens_reconciles_instead_of_refunding() -> TestResult {
		let ledger = BudgetLedger::spawn();
		let mut guard = guard_with_reservation(&ledger).await?;
		guard.input_tokens = 10;
		guard.output_tokens = 20;
		drop(guard);
		tokio::time::sleep(Duration::from_millis(800)).await;
		let counts = ledger.state_counts().await?;
		assert_eq!(counts.reservations, 1);
		Ok(())
	}

	#[tokio::test]
	async fn provider_cost_is_summed_over_iterations_and_dropped_when_one_is_missing() -> TestResult {
		let mut guard = guard_with_reservation(&BudgetLedger::spawn()).await?;
		assert_eq!(guard.provider_cost(), None);
		guard.start_iteration();
		guard.add_provider_cost(0.5);
		guard.start_iteration();
		guard.add_provider_cost(0.25);
		assert_eq!(guard.provider_cost(), Some(0.75));
		guard.start_iteration();
		assert_eq!(guard.provider_cost(), None);
		Ok(())
	}
}
