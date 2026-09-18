use super::*;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const SCALE: i128 = 1_000_000_000;

#[derive(Clone, Debug)]
/// Channel handle for the process-local admission owner.
pub struct GatewayLimiter {
	admissions: mpsc::Sender<Admission>,
	control: mpsc::UnboundedSender<Control>,
}

#[derive(Debug)]
/// Releases a concurrency lease without blocking when dropped.
pub struct GatewayReservation {
	pub id: Uuid,
	control: mpsc::UnboundedSender<Control>,
}

impl Drop for GatewayReservation {
	fn drop(&mut self) {
		let _ = self.control.send(Control::Release(self.id));
	}
}

#[derive(Debug)]
/// An accepted reservation and protocol-neutral post-admission capacities.
pub struct GatewayAdmission {
	pub reservation: GatewayReservation,
	pub snapshot: GatewayLimitSnapshot,
}

#[async_trait::async_trait]
/// Atomic admission and idempotent terminal usage accounting.
pub trait GatewayLimitBackend: Send + Sync {
	/// Reserves all applicable rules together.
	///
	/// # Errors
	/// Returns a rejection or unavailable error without committing partial charges.
	async fn admit(&self, request: GatewayAdmissionRequest) -> Result<GatewayAdmission, GatewayLimitError>;
	/// Applies the first final usage report for a reservation without blocking.
	fn reconcile(&self, reservation: Uuid, actual_tokens: u64);
}

impl GatewayLimiter {
	#[must_use]
	/// Starts the single-owner worker on the current Tokio runtime.
	pub fn spawn() -> Self {
		let (admissions, receiver) = mpsc::channel(256);
		let (control, controls) = mpsc::unbounded_channel();
		tokio::spawn(Owner::default().run(receiver, controls));
		Self { admissions, control }
	}

	#[cfg(test)]
	pub fn unavailable() -> Self {
		let (admissions, _) = mpsc::channel(1);
		let (control, _) = mpsc::unbounded_channel();
		Self { admissions, control }
	}

	#[cfg(test)]
	pub async fn state_counts(&self) -> Result<(usize, usize, usize), GatewayLimitError> {
		let (reply, response) = oneshot::channel();
		self.control.send(Control::Inspect(reply)).map_err(|_| GatewayLimitError::Unavailable)?;
		response.await.map_err(|_| GatewayLimitError::Unavailable)
	}
}

#[async_trait::async_trait]
impl GatewayLimitBackend for GatewayLimiter {
	async fn admit(&self, request: GatewayAdmissionRequest) -> Result<GatewayAdmission, GatewayLimitError> {
		request.policy.validate()?;
		let limited = request.policy.applies(request.route_class);
		let reservation = GatewayReservation {
			id: Uuid::new_v4(),
			control: self.control.clone(),
		};
		let (reply, response) = oneshot::channel();
		let message = Admission { request, reservation, reply };
		if let Err(error) = self.admissions.try_send(message) {
			return if limited {
				Err(GatewayLimitError::Unavailable)
			} else {
				Ok(GatewayAdmission {
					reservation: error.into_inner().reservation,
					snapshot: GatewayLimitSnapshot::default(),
				})
			};
		}
		response.await.map_err(|_| GatewayLimitError::Unavailable)?
	}

	fn reconcile(&self, reservation: Uuid, actual_tokens: u64) {
		if self.control.send(Control::Reconcile(reservation, actual_tokens)).is_err() {
			tracing::error!(%reservation, "gateway limiter stopped before usage reconciliation");
		}
	}
}

struct Admission {
	request: GatewayAdmissionRequest,
	reservation: GatewayReservation,
	reply: oneshot::Sender<Result<GatewayAdmission, GatewayLimitError>>,
}

#[derive(Debug)]
enum Control {
	Release(Uuid),
	Reconcile(Uuid, u64),
	#[cfg(test)]
	Inspect(oneshot::Sender<(usize, usize, usize)>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Subject {
	Project(Uuid),
	Key(Uuid),
}

type BucketKey = (Subject, Uuid);

struct Bucket {
	capacity: i128,
	period: i128,
	credit: i128,
	updated: Instant,
	remainder: i128,
}

impl Bucket {
	fn new(rule: &GatewayRateRule, now: Instant) -> Self {
		Self {
			capacity: i128::from(rule.capacity),
			period: i128::from(rule.refill_period_seconds),
			credit: i128::from(rule.capacity) * SCALE,
			updated: now,
			remainder: 0,
		}
	}

	fn refill(&mut self, now: Instant) {
		let elapsed = now.duration_since(self.updated).as_nanos().min(i128::MAX as u128) as i128;
		let refill = elapsed.saturating_mul(self.capacity).saturating_add(self.remainder);
		self.credit = self.credit.saturating_add(refill / self.period).min(self.capacity * SCALE);
		self.remainder = if self.credit == self.capacity * SCALE { 0 } else { refill % self.period };
		self.updated = now;
	}

	fn configure(&mut self, rule: &GatewayRateRule, now: Instant) {
		self.refill(now);
		if self.period != i128::from(rule.refill_period_seconds) {
			self.remainder = 0;
		}
		self.capacity = i128::from(rule.capacity);
		self.period = i128::from(rule.refill_period_seconds);
		self.credit = self.credit.min(self.capacity * SCALE);
	}

	fn wait(&self, credit: i128) -> Duration {
		let numerator = credit.max(0).saturating_mul(self.period).saturating_sub(self.remainder).max(0);
		let nanos = numerator / self.capacity + i128::from(numerator % self.capacity != 0);
		if nanos / SCALE > i128::from(u64::MAX) {
			Duration::MAX
		} else {
			Duration::new((nanos / SCALE) as u64, (nanos % SCALE) as u32)
		}
	}

	fn snapshot(&self, rule_id: Uuid) -> GatewayMetricSnapshot {
		GatewayMetricSnapshot {
			rule_id,
			capacity: self.capacity as u64,
			remaining: (self.credit.max(0) / SCALE) as u64,
			reset_after: self.wait(self.capacity * SCALE - self.credit),
		}
	}
}

struct Record {
	subjects: Option<[Subject; 2]>,
	buckets: Vec<BucketKey>,
	estimated: u64,
	reconciled: bool,
}

#[derive(Default)]
struct Owner {
	buckets: HashMap<BucketKey, Bucket>,
	concurrency: HashMap<Subject, u32>,
	reservations: HashMap<Uuid, Record>,
}

impl Owner {
	async fn run(mut self, mut admissions: mpsc::Receiver<Admission>, mut controls: mpsc::UnboundedReceiver<Control>) {
		let mut eviction = tokio::time::interval(Duration::from_secs(60));
		loop {
			tokio::select! {
				Some(control) = controls.recv() => self.control(control),
				message = admissions.recv() => match message {
					Some(message) => {
						if message.reply.is_closed() { continue; }
						while let Ok(control) = controls.try_recv() { self.control(control); }
						let result = self.admit(&message.request, message.reservation);
						let _ = message.reply.send(result);
					},
					None => break,
				},
				_ = eviction.tick() => self.evict(),
			}
		}
	}

	fn rules(request: &GatewayAdmissionRequest) -> impl Iterator<Item = (BucketKey, &GatewayRateRule)> {
		request
			.policy
			.project_rules
			.iter()
			.map(|rule| ((Subject::Project(request.project_id), rule.id), rule))
			.chain(request.policy.key_rules.iter().map(|rule| ((Subject::Key(request.key_id), rule.id), rule)))
			.filter(|(_, rule)| rule.matches(request.route_class))
	}

	fn admit(&mut self, request: &GatewayAdmissionRequest, reservation: GatewayReservation) -> Result<GatewayAdmission, GatewayLimitError> {
		let now = Instant::now();
		let mut failure = None;
		let mut retry_after = Duration::ZERO;
		for (key, rule) in Self::rules(request) {
			let bucket = self.buckets.entry(key).or_insert_with(|| Bucket::new(rule, now));
			bucket.configure(rule, now);
			let cost = Self::cost(rule, request.tokens);
			if bucket.credit < cost {
				failure.get_or_insert(rule.metric);
				retry_after = retry_after.max(if cost > bucket.capacity * SCALE {
					Duration::MAX
				} else {
					bucket.wait(cost - bucket.credit)
				});
			}
		}
		let subjects = [Subject::Project(request.project_id), Subject::Key(request.key_id)];
		let caps = [request.policy.project_concurrency, request.policy.key_concurrency];
		if request.route_class == GatewayRouteClass::Inference {
			for (subject, cap) in subjects.iter().zip(caps) {
				if cap.is_some_and(|cap| self.concurrency.get(subject).copied().unwrap_or(0) >= cap) {
					failure.get_or_insert(GatewayLimitDimension::Concurrency);
					retry_after = retry_after.max(Duration::from_secs(1));
				}
			}
		}
		if let Some(dimension) = failure {
			return Err(GatewayLimitError::Rejected {
				dimension,
				snapshot: self.snapshot(request),
				retry_after,
			});
		}
		let mut token_buckets = Vec::with_capacity(request.policy.project_rules.len() + request.policy.key_rules.len());
		for (key, rule) in Self::rules(request) {
			if let Some(bucket) = self.buckets.get_mut(&key) {
				bucket.credit -= Self::cost(rule, request.tokens);
			}
			if rule.metric == GatewayLimitDimension::Tokens {
				token_buckets.push(key);
			}
		}
		let subjects = if request.route_class == GatewayRouteClass::Inference {
			for subject in subjects {
				let count = self.concurrency.entry(subject).or_default();
				*count = count.saturating_add(1);
			}
			Some(subjects)
		} else {
			None
		};
		self.reservations.insert(
			reservation.id,
			Record {
				subjects,
				buckets: token_buckets,
				estimated: request.tokens,
				reconciled: false,
			},
		);
		Ok(GatewayAdmission {
			reservation,
			snapshot: self.snapshot(request),
		})
	}

	fn cost(rule: &GatewayRateRule, tokens: u64) -> i128 {
		i128::from(if rule.metric == GatewayLimitDimension::Requests { 1 } else { tokens }) * SCALE
	}

	fn snapshot(&self, request: &GatewayAdmissionRequest) -> GatewayLimitSnapshot {
		let mut snapshot = GatewayLimitSnapshot::default();
		let mut selected_requests: Option<(Uuid, &Bucket)> = None;
		let mut selected_tokens: Option<(Uuid, &Bucket)> = None;
		for (key, rule) in Self::rules(request) {
			let Some(bucket) = self.buckets.get(&key) else {
				continue;
			};
			let target = if rule.metric == GatewayLimitDimension::Requests {
				&mut selected_requests
			} else {
				&mut selected_tokens
			};
			if target.is_none_or(|(id, current)| {
				let ordering = ratio_cmp(
					(bucket.credit.max(0) as u128, bucket.capacity as u128),
					(current.credit.max(0) as u128, current.capacity as u128),
				);
				ordering.is_lt() || (ordering.is_eq() && rule.id < id)
			}) {
				*target = Some((rule.id, bucket));
			}
		}
		snapshot.requests = selected_requests.map(|(id, bucket)| bucket.snapshot(id));
		snapshot.tokens = selected_tokens.map(|(id, bucket)| bucket.snapshot(id));
		if request.route_class == GatewayRouteClass::Inference {
			for (subject, cap) in [
				(Subject::Project(request.project_id), request.policy.project_concurrency),
				(Subject::Key(request.key_id), request.policy.key_concurrency),
			] {
				if let Some(cap) = cap {
					let remaining = cap.saturating_sub(self.concurrency.get(&subject).copied().unwrap_or(0));
					if snapshot
						.concurrency
						.is_none_or(|(limit, left)| u64::from(remaining) * u64::from(limit) < u64::from(left) * u64::from(cap))
					{
						snapshot.concurrency = Some((cap, remaining));
					}
				}
			}
		}
		snapshot
	}

	fn control(&mut self, command: Control) {
		let id = match command {
			Control::Release(id) | Control::Reconcile(id, _) => id,
			#[cfg(test)]
			Control::Inspect(reply) => {
				let _ = reply.send((self.buckets.len(), self.concurrency.len(), self.reservations.len()));
				return;
			}
		};
		let Some(record) = self.reservations.get_mut(&id) else {
			return;
		};
		match command {
			Control::Release(_) => {
				if let Some(subjects) = record.subjects.take() {
					for subject in subjects {
						if let Some(count) = self.concurrency.get_mut(&subject) {
							*count = count.saturating_sub(1);
						}
					}
				}
			}
			Control::Reconcile(_, actual) if !record.reconciled => {
				let adjustment = (i128::from(record.estimated) - i128::from(actual)) * SCALE;
				for key in &record.buckets {
					if let Some(bucket) = self.buckets.get_mut(key) {
						bucket.refill(Instant::now());
						bucket.credit = bucket.credit.saturating_add(adjustment).min(bucket.capacity * SCALE);
					}
				}
				record.reconciled = true;
			}
			Control::Reconcile(_, _) => {}
			#[cfg(test)]
			Control::Inspect(_) => {}
		}
		if record.subjects.is_none() && (record.reconciled || record.buckets.is_empty()) {
			self.reservations.remove(&id);
		}
	}

	fn evict(&mut self) {
		let now = Instant::now();
		self.buckets.values_mut().for_each(|bucket| bucket.refill(now));
		self.reservations.retain(|_, record| {
			record.subjects.is_some()
				|| record
					.buckets
					.iter()
					.any(|key| self.buckets.get(key).is_some_and(|bucket| bucket.credit < bucket.capacity * SCALE))
		});
		let retained: std::collections::HashSet<BucketKey> = self.reservations.values().flat_map(|record| record.buckets.iter().copied()).collect();
		self.buckets
			.retain(|key, bucket| bucket.credit < bucket.capacity * SCALE || retained.contains(key) || self.concurrency.get(&key.0).is_some_and(|count| *count > 0));
		self.concurrency.retain(|_, count| *count != 0);
	}
}

fn ratio_cmp(mut left: (u128, u128), mut right: (u128, u128)) -> std::cmp::Ordering {
	let mut reverse = false;
	loop {
		let order = (left.0 / left.1).cmp(&(right.0 / right.1));
		let remainders = (left.0 % left.1, right.0 % right.1);
		let order = if order.is_eq() && (remainders.0 == 0 || remainders.1 == 0) {
			remainders.0.cmp(&remainders.1)
		} else {
			order
		};
		if !order.is_eq() || remainders.0 == 0 || remainders.1 == 0 {
			return if reverse { order.reverse() } else { order };
		}
		left = (left.1, remainders.0);
		right = (right.1, remainders.1);
		reverse = !reverse;
	}
}
