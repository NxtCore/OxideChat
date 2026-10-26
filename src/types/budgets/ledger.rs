use super::Budget;
use crate::types::models::ModelPricing;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use uuid::Uuid;

const ADMISSION_QUEUE: usize = 256;
const SETTLED_WINDOW: Duration = Duration::from_secs(60);
const SETTLE_TIMEOUT: Duration = Duration::from_secs(300);
const EVICTION_INTERVAL: Duration = Duration::from_secs(60);

// Request Types

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Who a request is spending for; a missing user is a service-account key, which also carries its team.
pub struct BudgetSubject {
	pub user_id: Option<Uuid>,
	pub api_key_id: Option<Uuid>,
	pub service_team_id: Option<Uuid>,
}

impl BudgetSubject {
	#[must_use]
	/// A request made by a user without an API key, such as web chat.
	pub fn user(user_id: Uuid) -> Self {
		Self {
			user_id: Some(user_id),
			api_key_id: None,
			service_team_id: None,
		}
	}

	#[must_use]
	/// A request made by a user through one of their API keys.
	pub fn user_key(user_id: Uuid, api_key_id: Uuid) -> Self {
		Self {
			user_id: Some(user_id),
			api_key_id: Some(api_key_id),
			service_team_id: None,
		}
	}

	#[must_use]
	/// Only the budgets assigned to one key.
	pub fn key(api_key_id: Uuid) -> Self {
		Self {
			user_id: None,
			api_key_id: Some(api_key_id),
			service_team_id: None,
		}
	}

	#[must_use]
	/// A request made by a service-account key: its own budgets and its team's pooled budgets.
	pub fn service_key(team_id: Uuid, api_key_id: Uuid) -> Self {
		Self {
			user_id: None,
			api_key_id: Some(api_key_id),
			service_team_id: Some(team_id),
		}
	}
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Conservative token bounds used to price a request before it runs.
pub struct TokenEstimate {
	pub input: u64,
	pub output: u64,
}

impl TokenEstimate {
	#[must_use]
	/// Conservative input tokens for a request body of the given size.
	pub fn input_for_bytes(bytes: u64) -> u64 {
		bytes.saturating_mul(2).saturating_add(1024)
	}

	#[must_use]
	/// Input and output tokens together.
	pub fn total(&self) -> u64 {
		self.input.saturating_add(self.output)
	}
}

#[derive(Clone, Copy, Debug)]
/// Estimated cost of one request for one subject.
pub struct BudgetRequest {
	pub subject: BudgetSubject,
	pub estimated_cost: Decimal,
}

// Response Types

#[derive(Debug, Default)]
/// Outcome of a successful budget admission.
pub struct BudgetAdmission {
	pub reservation: Option<BudgetReservation>,
	pub warn: bool,
}

/// Failures produced while admitting a request against budgets.
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
	/// A blocking budget has no remaining room.
	#[error("budget_exceeded")]
	Exceeded,
	/// The reservation owner is not running.
	#[error("budget_unavailable")]
	Unavailable,
	/// Budget or usage storage could not be queried.
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

// Internal Types

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// The spend a reservation competes for: one assignment, narrowed to a user when spend is per user.
pub struct BudgetScope {
	pub assignment_id: Uuid,
	pub user_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// What an exhausted budget does to a request.
pub enum BudgetAction {
	Block,
	Warn,
	Allow,
}

impl BudgetAction {
	#[must_use]
	/// Parses the stored `on_exceed` value; unknown values do not enforce anything.
	pub fn from_stored(value: &str) -> Self {
		match value {
			"block" => Self::Block,
			"warn" => Self::Warn,
			_ => Self::Allow,
		}
	}
}

#[derive(Clone, Copy, Debug)]
/// One effective budget with the spend already stored for it.
pub struct BudgetCheck {
	pub scope: BudgetScope,
	pub amount: Decimal,
	pub spent: Decimal,
	pub action: BudgetAction,
}

#[derive(Clone, Debug)]
/// Channel handle for the process-local budget reservation owner.
pub struct BudgetLedger {
	admissions: mpsc::Sender<Admit>,
	control: mpsc::UnboundedSender<Control>,
	epoch: Arc<AtomicU64>,
}

#[derive(Debug)]
/// Refunds an unfinished reservation when dropped.
pub struct BudgetReservation {
	pub id: Uuid,
	control: mpsc::UnboundedSender<Control>,
}

impl Drop for BudgetReservation {
	fn drop(&mut self) {
		let _ = self.control.send(Control::Release(self.id));
	}
}

struct Admit {
	reservation: BudgetReservation,
	checks: Vec<BudgetCheck>,
	estimated_cost: Decimal,
	epoch: u64,
	reply: oneshot::Sender<Reply>,
}

enum Reply {
	Admitted(BudgetAdmission),
	Exceeded,
}

#[derive(Debug)]
enum Control {
	Release(Uuid),
	Finalize(Uuid),
	Reconcile(Uuid, Decimal),
	Settle(Uuid, u64),
	#[cfg(test)]
	Inspect(oneshot::Sender<BudgetStateCounts>),
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
/// Sizes of the ledger's internal tables.
pub struct BudgetStateCounts {
	pub scopes: usize,
	pub reservations: usize,
}

struct Record {
	scopes: Vec<BudgetScope>,
	amount: Decimal,
	finalized: Option<Instant>,
	reconciled: bool,
}

struct Settled {
	epoch: u64,
	scopes: Vec<BudgetScope>,
	amount: Decimal,
	at: Instant,
}

struct Owner {
	settled: Vec<Settled>,
	reserved: HashMap<BudgetScope, Decimal>,
	records: HashMap<Uuid, Record>,
}

impl BudgetLedger {
	#[must_use]
	/// Starts the single-owner worker on the current Tokio runtime.
	pub fn spawn() -> Self {
		let (admissions, receiver) = mpsc::channel(ADMISSION_QUEUE);
		let (control, controls) = mpsc::unbounded_channel();
		let epoch = Arc::new(AtomicU64::new(0));
		let owner = Owner {
			settled: Vec::new(),
			reserved: HashMap::new(),
			records: HashMap::new(),
		};
		tokio::spawn(owner.run(receiver, controls));
		Self { admissions, control, epoch }
	}

	#[cfg(test)]
	pub fn unavailable() -> Self {
		let (admissions, _) = mpsc::channel(1);
		let (control, _) = mpsc::unbounded_channel();
		Self {
			admissions,
			control,
			epoch: Arc::default(),
		}
	}

	#[cfg(test)]
	pub async fn state_counts(&self) -> Result<BudgetStateCounts, BudgetError> {
		let (reply, response) = oneshot::channel();
		self.control.send(Control::Inspect(reply)).map_err(|_| BudgetError::Unavailable)?;
		response.await.map_err(|_| BudgetError::Unavailable)
	}

	/// Admits one request against every budget effective for the subject and reserves its estimated cost.
	///
	/// Blocking budgets reject once stored spend plus in-flight reservations reaches their amount,
	/// so concurrent requests overshoot a hard limit by at most one estimate. Warning budgets set
	/// [`BudgetAdmission::warn`]. The reservation is refunded when dropped unless usage was reported.
	///
	/// # Errors
	/// Returns [`BudgetError::Exceeded`] when a blocking budget is exhausted, [`BudgetError::Unavailable`]
	/// when the reservation owner stopped, and a database error when budget status cannot be loaded.
	pub async fn reserve(&self, pool: &PgPool, request: BudgetRequest) -> Result<BudgetAdmission, BudgetError> {
		let epoch = self.epoch.load(Ordering::Acquire);
		let checks = Budget::checks(pool, request.subject).await?;
		self.admit_at(checks, request.estimated_cost, epoch).await
	}

	/// Prices the estimate for a model and reserves it; free models bypass budgets.
	///
	/// # Errors
	/// Returns the errors of [`BudgetLedger::reserve`] and a database error when pricing cannot be loaded.
	pub async fn reserve_for_model(&self, pool: &PgPool, subject: BudgetSubject, model_id: &Uuid, tokens: TokenEstimate) -> Result<BudgetAdmission, BudgetError> {
		if ModelPricing::is_free(pool, model_id).await? {
			return Ok(BudgetAdmission::default());
		}
		let estimated_cost = ModelPricing::estimate_cost(pool, model_id, tokens).await?;
		self.reserve(pool, BudgetRequest { subject, estimated_cost }).await
	}

	/// Admits against already loaded budget checks.
	///
	/// # Errors
	/// Returns [`BudgetError::Exceeded`] or [`BudgetError::Unavailable`].
	pub async fn admit_checks(&self, checks: Vec<BudgetCheck>, estimated_cost: Decimal) -> Result<BudgetAdmission, BudgetError> {
		self.admit_at(checks, estimated_cost, self.epoch()).await
	}

	#[must_use]
	/// Settle counter to read before loading budget checks.
	pub fn epoch(&self) -> u64 {
		self.epoch.load(Ordering::Acquire)
	}

	/// Admits against budget checks that were loaded after the given epoch was read.
	///
	/// Usage settled after that epoch may or may not be part of the loaded spend, so the
	/// ledger counts it again; this can only reject early, never overshoot.
	///
	/// # Errors
	/// Returns [`BudgetError::Exceeded`] or [`BudgetError::Unavailable`].
	pub async fn admit_at(&self, checks: Vec<BudgetCheck>, estimated_cost: Decimal, epoch: u64) -> Result<BudgetAdmission, BudgetError> {
		match self.submit(checks, estimated_cost, epoch).await? {
			Reply::Admitted(admission) => Ok(admission),
			Reply::Exceeded => Err(BudgetError::Exceeded),
		}
	}

	/// Marks that usage for a reservation was reported, so dropping its guard no longer refunds it.
	pub fn finalize(&self, id: Uuid) {
		self.send(Control::Finalize(id));
	}

	/// Replaces the estimate with the actual cost once; later calls are ignored.
	pub fn reconcile(&self, id: Uuid, actual_cost: Decimal) {
		self.send(Control::Reconcile(id, actual_cost));
	}

	/// Drops a reservation after its usage was stored, where stored spend now counts instead.
	pub fn settle(&self, id: Uuid) {
		let epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
		self.send(Control::Settle(id, epoch));
	}

	fn send(&self, control: Control) {
		if self.control.send(control).is_err() {
			tracing::error!("budget ledger stopped before a reservation update");
		}
	}

	async fn submit(&self, checks: Vec<BudgetCheck>, estimated_cost: Decimal, epoch: u64) -> Result<Reply, BudgetError> {
		let reservation = BudgetReservation {
			id: Uuid::new_v4(),
			control: self.control.clone(),
		};
		let (reply, response) = oneshot::channel();
		let message = Admit {
			reservation,
			checks,
			estimated_cost,
			epoch,
			reply,
		};
		self.admissions.send(message).await.map_err(|_| BudgetError::Unavailable)?;
		response.await.map_err(|_| BudgetError::Unavailable)
	}
}

impl Owner {
	async fn run(mut self, mut admissions: mpsc::Receiver<Admit>, mut controls: mpsc::UnboundedReceiver<Control>) {
		let mut eviction = tokio::time::interval(EVICTION_INTERVAL);
		loop {
			tokio::select! {
				Some(control) = controls.recv() => self.control(control),
				message = admissions.recv() => match message {
					Some(message) => self.serve(message, &mut controls),
					None => break,
				},
				_ = eviction.tick() => self.evict(),
			}
		}
	}

	fn serve(&mut self, message: Admit, controls: &mut mpsc::UnboundedReceiver<Control>) {
		while let Ok(control) = controls.try_recv() {
			self.control(control);
		}
		let reply = message.reply;
		let outcome = self.admit(message.reservation, &message.checks, message.estimated_cost, message.epoch);
		let _ = reply.send(outcome);
	}

	fn admit(&mut self, reservation: BudgetReservation, checks: &[BudgetCheck], estimated_cost: Decimal, epoch: u64) -> Reply {
		let mut warn = false;
		for check in checks {
			if check.spent + self.reserved_for(&check.scope) + self.settled_since(&check.scope, epoch) < check.amount {
				continue;
			}
			match check.action {
				BudgetAction::Block => return Reply::Exceeded,
				BudgetAction::Warn => warn = true,
				BudgetAction::Allow => {}
			}
		}
		let scopes: Vec<BudgetScope> = checks.iter().filter(|check| check.action != BudgetAction::Allow).map(|check| check.scope).collect();
		for scope in &scopes {
			*self.reserved.entry(*scope).or_default() += estimated_cost;
		}
		self.records.insert(
			reservation.id,
			Record {
				scopes,
				amount: estimated_cost,
				finalized: None,
				reconciled: false,
			},
		);
		Reply::Admitted(BudgetAdmission {
			reservation: Some(reservation),
			warn,
		})
	}

	fn settled_since(&self, scope: &BudgetScope, epoch: u64) -> Decimal {
		self.settled
			.iter()
			.filter(|settled| settled.epoch > epoch && settled.scopes.contains(scope))
			.map(|settled| settled.amount)
			.sum()
	}

	fn reserved_for(&self, scope: &BudgetScope) -> Decimal {
		self.reserved.get(scope).copied().unwrap_or(Decimal::ZERO)
	}

	fn control(&mut self, control: Control) {
		match control {
			Control::Release(id) => {
				if self.records.get(&id).is_some_and(|record| record.finalized.is_none()) {
					self.remove(id);
				}
			}
			Control::Finalize(id) => {
				if let Some(record) = self.records.get_mut(&id) {
					record.finalized.get_or_insert_with(Instant::now);
				}
			}
			Control::Reconcile(id, actual) => self.reconcile(id, actual),
			Control::Settle(id, epoch) => self.settle(id, epoch),
			#[cfg(test)]
			Control::Inspect(reply) => {
				let _ = reply.send(BudgetStateCounts {
					scopes: self.reserved.len(),
					reservations: self.records.len(),
				});
			}
		}
	}

	fn reconcile(&mut self, id: Uuid, actual: Decimal) {
		let Some(record) = self.records.get_mut(&id).filter(|record| !record.reconciled) else {
			return;
		};
		let adjustment = actual - record.amount;
		for scope in &record.scopes {
			let reserved = self.reserved.entry(*scope).or_default();
			*reserved = (*reserved + adjustment).max(Decimal::ZERO);
		}
		record.amount = actual;
		record.reconciled = true;
		record.finalized.get_or_insert_with(Instant::now);
	}

	fn settle(&mut self, id: Uuid, epoch: u64) {
		if let Some(record) = self.records.get(&id) {
			self.settled.push(Settled {
				epoch,
				scopes: record.scopes.clone(),
				amount: record.amount,
				at: Instant::now(),
			});
		}
		self.remove(id);
	}

	fn remove(&mut self, id: Uuid) {
		let Some(record) = self.records.remove(&id) else {
			return;
		};
		for scope in record.scopes {
			if let Some(reserved) = self.reserved.get_mut(&scope) {
				*reserved = (*reserved - record.amount).max(Decimal::ZERO);
				if reserved.is_zero() {
					self.reserved.remove(&scope);
				}
			}
		}
	}

	fn evict(&mut self) {
		self.settled.retain(|settled| settled.at.elapsed() < SETTLED_WINDOW);
		let expired: Vec<Uuid> = self
			.records
			.iter()
			.filter(|(_, record)| record.finalized.is_some_and(|finalized| finalized.elapsed() >= SETTLE_TIMEOUT))
			.map(|(id, _)| *id)
			.collect();
		for id in expired {
			self.remove(id);
		}
	}
}
