use super::{UsageEvent, UsageEventRecord};
use crate::types::models::ModelPricing;
use crate::types::{Budget, BudgetReservation, JobState};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

/// Records the usage of a chat stream once, even when the client disconnects before it ends.
pub struct ChatUsageGuard {
	state: Arc<JobState>,
	reservation: Option<BudgetReservation>,
	user_id: Uuid,
	model_id: Uuid,
	provider_id: Uuid,
	pub input_tokens: u32,
	pub output_tokens: u32,
	pub reasoning_tokens: u32,
	iterations: u32,
	costed_iterations: u32,
	provider_cost_sum: f64,
	recorded: bool,
}

impl ChatUsageGuard {
	/// Starts tracking a stream that holds the given budget reservation.
	#[must_use]
	pub fn new(state: Arc<JobState>, reservation: Option<BudgetReservation>, user_id: Uuid, model_id: Uuid, provider_id: Uuid) -> Self {
		Self {
			state,
			reservation,
			user_id,
			model_id,
			provider_id,
			input_tokens: 0,
			output_tokens: 0,
			reasoning_tokens: 0,
			iterations: 0,
			costed_iterations: 0,
			provider_cost_sum: 0.0,
			recorded: false,
		}
	}

	/// Counts one more request of the agent loop.
	pub fn start_iteration(&mut self) {
		self.iterations += 1;
	}

	/// Adds the cost the provider reported for one request of the agent loop.
	pub fn add_provider_cost(&mut self, cost: f64) {
		self.costed_iterations += 1;
		self.provider_cost_sum += cost;
	}

	/// The summed provider cost, or `None` when any request reported no cost so all tokens are priced locally.
	#[must_use]
	pub fn provider_cost(&self) -> Option<f64> {
		(self.iterations > 0 && self.costed_iterations == self.iterations).then_some(self.provider_cost_sum)
	}

	/// Prices the tokens, reconciles the reservation to that cost and stores the usage event.
	///
	/// # Errors
	/// Returns an error when the usage event cannot be stored.
	pub async fn record(&mut self) -> Result<(), sqlx::Error> {
		self.recorded = true;
		let db = &self.state.db;
		let mut event = UsageEventRecord {
			user_id: Some(&self.user_id),
			team_id: Budget::primary_team_id(db, &self.user_id).await.ok().flatten(),
			api_key_id: None,
			model_id: &self.model_id,
			provider_id: &self.provider_id,
			request_type: "chat",
			input_tokens: i32::try_from(self.input_tokens).unwrap_or(i32::MAX),
			output_tokens: i32::try_from(self.output_tokens).unwrap_or(i32::MAX),
			reasoning_tokens: i32::try_from(self.reasoning_tokens).unwrap_or(i32::MAX),
			cost_total: Decimal::ZERO,
		};
		if let Err(error) = ModelPricing::price(db, self.provider_cost(), 0.0, &mut event).await {
			tracing::error!(%error, model_id = %self.model_id, "failed to price chat usage");
		}
		if let Some(reservation) = &self.reservation {
			self.state.budget_ledger.reconcile(reservation.id, event.cost_total);
		}
		UsageEvent::record(db, event).await?;
		if let Some(reservation) = &self.reservation {
			self.state.budget_ledger.settle(reservation.id);
		}
		Ok(())
	}
}

impl Drop for ChatUsageGuard {
	fn drop(&mut self) {
		let consumed = self.input_tokens > 0 || self.output_tokens > 0 || self.reasoning_tokens > 0;
		if self.recorded || !consumed {
			return;
		}
		let Ok(runtime) = tokio::runtime::Handle::try_current() else {
			return;
		};
		let mut interrupted = Self {
			state: Arc::clone(&self.state),
			reservation: self.reservation.take(),
			user_id: self.user_id,
			model_id: self.model_id,
			provider_id: self.provider_id,
			input_tokens: self.input_tokens,
			output_tokens: self.output_tokens,
			reasoning_tokens: self.reasoning_tokens,
			iterations: self.iterations,
			costed_iterations: self.costed_iterations,
			provider_cost_sum: self.provider_cost_sum,
			recorded: false,
		};
		runtime.spawn(async move {
			if let Err(error) = interrupted.record().await {
				tracing::error!(%error, "failed to record interrupted chat usage");
			}
		});
	}
}
