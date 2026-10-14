use super::body::ReservedBody;
use super::{
	GatewayAdmissionRequest, GatewayLimitDimension, GatewayLimitError, GatewayLimitSnapshot, GatewayMetricSnapshot, GatewayRateRule, GatewayRouteClass,
	GatewayRuleMetric,
};
use axum::body::Body;
use axum::response::Response;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use uuid::Uuid;

const SCALE: i128 = 1_000_000_000;
const ADMISSION_QUEUE: usize = 256;
const CONCURRENCY_RETRY: Duration = Duration::from_secs(1);

// Response Types

#[derive(Debug)]
/// An accepted reservation and protocol-neutral post-admission capacities.
pub struct GatewayAdmission {
	pub reservation: GatewayReservation,
	pub snapshot: GatewayLimitSnapshot,
}

// Internal Types

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
/// Sizes of the limiter's internal tables.
pub struct GatewayStateCounts {
	pub buckets: usize,
	pub concurrency: usize,
	pub reservations: usize,
}

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

impl GatewayReservation {
	#[must_use]
	/// Holds this reservation until the response body completes, fails, or is dropped.
	pub fn attach(self, response: Response) -> Response {
		let (parts, body) = response.into_parts();
		Response::from_parts(parts, Body::new(ReservedBody::new(body, self)))
	}
}

impl Drop for GatewayReservation {
	fn drop(&mut self) {
		let _ = self.control.send(Control::Release(self.id));
	}
}

#[async_trait::async_trait]
/// Atomic admission and idempotent terminal usage accounting.
pub trait GatewayLimitBackend: Send + Sync {
	/// Reserves all applicable rules together.
	///
	/// # Errors
	/// Returns a rejection or an enforcement error without committing partial charges.
	async fn admit(&self, request: GatewayAdmissionRequest) -> Result<GatewayAdmission, GatewayLimitError>;
	/// Applies the first final usage report for a reservation without blocking.
	fn reconcile(&self, reservation: Uuid, actual_tokens: u64);
}

impl GatewayLimiter {
	#[must_use]
	/// Starts the single-owner worker on the current Tokio runtime.
	pub fn spawn() -> Self {
		let (admissions, receiver) = mpsc::channel(ADMISSION_QUEUE);
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
	pub fn saturated() -> Self {
		let (admissions, receiver) = mpsc::channel(1);
		let (control, _) = mpsc::unbounded_channel();
		let (reply, _) = oneshot::channel();
		let request = GatewayAdmissionRequest {
			team_id: Uuid::nil(),
			key_id: Uuid::nil(),
			policy: Arc::default(),
			route_class: GatewayRouteClass::Inference,
			tokens: 0,
		};
		let reservation = GatewayReservation {
			id: Uuid::nil(),
			control: control.clone(),
		};
		let _ = admissions.try_send(Admission { request, reservation, reply });
		tokio::spawn(async move {
			let _receiver = receiver;
			std::future::pending::<()>().await;
		});
		Self { admissions, control }
	}

	#[cfg(test)]
	pub async fn state_counts(&self) -> Result<GatewayStateCounts, GatewayLimitError> {
		let (reply, response) = oneshot::channel();
		self.control.send(Control::Inspect(reply)).map_err(|_| GatewayLimitError::Unavailable)?;
		response.await.map_err(|_| GatewayLimitError::Unavailable)
	}

	fn bypass(error: TrySendError<Admission>, limited: bool) -> Result<GatewayAdmission, GatewayLimitError> {
		match (error, limited) {
			(TrySendError::Full(_), true) => Err(GatewayLimitError::Overloaded),
			(TrySendError::Closed(_), true) => Err(GatewayLimitError::Unavailable),
			(TrySendError::Full(message) | TrySendError::Closed(message), false) => Ok(GatewayAdmission {
				reservation: message.reservation,
				snapshot: GatewayLimitSnapshot::default(),
			}),
		}
	}
}

#[async_trait::async_trait]
impl GatewayLimitBackend for GatewayLimiter {
	async fn admit(&self, request: GatewayAdmissionRequest) -> Result<GatewayAdmission, GatewayLimitError> {
		let limited = request.policy.applies(request.route_class);
		let reservation = GatewayReservation {
			id: Uuid::new_v4(),
			control: self.control.clone(),
		};
		let (reply, response) = oneshot::channel();
		let message = Admission { request, reservation, reply };
		if let Err(error) = self.admissions.try_send(message) {
			return Self::bypass(error, limited);
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
	Inspect(oneshot::Sender<GatewayStateCounts>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Subject {
	Team(Uuid),
	Key(Uuid),
}

type BucketKey = (Subject, Uuid);

#[derive(Default)]
struct Failure {
	dimension: Option<GatewayLimitDimension>,
	retry_after: Duration,
}

impl Failure {
	fn add(&mut self, dimension: GatewayLimitDimension, retry_after: Duration) {
		self.dimension.get_or_insert(dimension);
		self.retry_after = self.retry_after.max(retry_after);
	}
}

struct Bucket {
	capacity: i128,
	period: i128,
	credit: i128,
	updated: Instant,
	remainder: i128,
}

impl Bucket {
	fn new(rule: &GatewayRateRule, now: Instant) -> Self {
		let capacity = i128::from(rule.capacity);
		Self {
			capacity,
			period: i128::from(rule.refill_period_seconds),
			credit: capacity * SCALE,
			updated: now,
			remainder: 0,
		}
	}

	fn full(&self) -> i128 {
		self.capacity * SCALE
	}

	fn is_full(&self) -> bool {
		self.credit >= self.full()
	}

	fn refill(&mut self, now: Instant) {
		let elapsed = i128::try_from(now.duration_since(self.updated).as_nanos()).unwrap_or(i128::MAX);
		let refill = elapsed.saturating_mul(self.capacity).saturating_add(self.remainder);
		self.credit = self.credit.saturating_add(refill / self.period).min(self.full());
		self.remainder = if self.is_full() { 0 } else { refill % self.period };
		self.updated = now;
	}

	fn configure(&mut self, rule: &GatewayRateRule, now: Instant) {
		self.refill(now);
		if self.period != i128::from(rule.refill_period_seconds) {
			self.remainder = 0;
		}
		self.capacity = i128::from(rule.capacity);
		self.period = i128::from(rule.refill_period_seconds);
		self.credit = self.credit.min(self.full());
	}

	fn wait(&self, credit: i128) -> Duration {
		let numerator = credit.max(0).saturating_mul(self.period).saturating_sub(self.remainder).max(0);
		let nanos = numerator / self.capacity + i128::from(numerator % self.capacity != 0);
		match (u64::try_from(nanos / SCALE), u32::try_from(nanos % SCALE)) {
			(Ok(seconds), Ok(subsec)) => Duration::new(seconds, subsec),
			_ => Duration::MAX,
		}
	}

	fn retry_after(&self, cost: i128) -> Duration {
		if cost > self.full() { Duration::MAX } else { self.wait(cost - self.credit) }
	}

	fn fill_cmp(&self, other: &Self) -> Ordering {
		let credits = (u128::try_from(self.credit.max(0)).unwrap_or(0), u128::try_from(other.credit.max(0)).unwrap_or(0));
		let capacities = (u128::try_from(self.capacity).unwrap_or(0), u128::try_from(other.capacity).unwrap_or(0));
		match (credits.0.checked_mul(capacities.1), credits.1.checked_mul(capacities.0)) {
			(Some(left), Some(right)) => left.cmp(&right),
			_ => {
				let scale = u128::try_from(SCALE).unwrap_or(1);
				(credits.0 / scale * capacities.1).cmp(&(credits.1 / scale * capacities.0))
			}
		}
	}

	fn snapshot(&self, rule_id: Uuid) -> GatewayMetricSnapshot {
		GatewayMetricSnapshot {
			rule_id,
			capacity: u64::try_from(self.capacity).unwrap_or(u64::MAX),
			remaining: u64::try_from(self.credit.max(0) / SCALE).unwrap_or(u64::MAX),
			reset_after: self.wait(self.full() - self.credit),
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
					Some(message) => self.serve(message, &mut controls),
					None => break,
				},
				_ = eviction.tick() => self.evict(),
			}
		}
	}

	fn serve(&mut self, message: Admission, controls: &mut mpsc::UnboundedReceiver<Control>) {
		if message.reply.is_closed() {
			return;
		}
		while let Ok(control) = controls.try_recv() {
			self.control(control);
		}
		let result = self.admit(&message.request, message.reservation);
		let _ = message.reply.send(result);
	}

	fn rules(request: &GatewayAdmissionRequest) -> impl Iterator<Item = (BucketKey, &GatewayRateRule)> {
		request
			.policy
			.team_rules
			.iter()
			.map(|rule| ((Subject::Team(request.team_id), rule.id), rule))
			.chain(request.policy.key_rules.iter().map(|rule| ((Subject::Key(request.key_id), rule.id), rule)))
			.filter(|(_, rule)| rule.matches(request.route_class))
	}

	fn caps(request: &GatewayAdmissionRequest) -> [(Subject, Option<u32>); 2] {
		[
			(Subject::Team(request.team_id), request.policy.team_concurrency),
			(Subject::Key(request.key_id), request.policy.key_concurrency),
		]
	}

	fn active(&self, subject: &Subject) -> u32 {
		self.concurrency.get(subject).copied().unwrap_or(0)
	}

	fn cost(rule: &GatewayRateRule, tokens: u64) -> i128 {
		i128::from(if rule.metric == GatewayRuleMetric::Requests { 1 } else { tokens }) * SCALE
	}

	fn admit(&mut self, request: &GatewayAdmissionRequest, reservation: GatewayReservation) -> Result<GatewayAdmission, GatewayLimitError> {
		let mut failure = Failure::default();
		self.check_rules(request, &mut failure);
		self.check_concurrency(request, &mut failure);
		if let Some(dimension) = failure.dimension {
			return Err(GatewayLimitError::Rejected {
				dimension,
				snapshot: Box::new(self.snapshot(request)),
				retry_after: failure.retry_after,
			});
		}
		let buckets = self.charge(request);
		let subjects = self.acquire(request);
		self.reservations.insert(
			reservation.id,
			Record {
				subjects,
				buckets,
				estimated: request.tokens,
				reconciled: false,
			},
		);
		Ok(GatewayAdmission {
			reservation,
			snapshot: self.snapshot(request),
		})
	}

	fn check_rules(&mut self, request: &GatewayAdmissionRequest, failure: &mut Failure) {
		let now = Instant::now();
		for (key, rule) in Self::rules(request) {
			let bucket = self.buckets.entry(key).or_insert_with(|| Bucket::new(rule, now));
			bucket.configure(rule, now);
			let cost = Self::cost(rule, request.tokens);
			if bucket.credit < cost {
				failure.add(rule.metric.into(), bucket.retry_after(cost));
			}
		}
	}

	fn check_concurrency(&self, request: &GatewayAdmissionRequest, failure: &mut Failure) {
		if request.route_class != GatewayRouteClass::Inference {
			return;
		}
		for (subject, cap) in Self::caps(request) {
			if cap.is_some_and(|cap| self.active(&subject) >= cap) {
				failure.add(GatewayLimitDimension::Concurrency, CONCURRENCY_RETRY);
			}
		}
	}

	fn charge(&mut self, request: &GatewayAdmissionRequest) -> Vec<BucketKey> {
		let mut token_buckets = Vec::with_capacity(request.policy.team_rules.len() + request.policy.key_rules.len());
		for (key, rule) in Self::rules(request) {
			if let Some(bucket) = self.buckets.get_mut(&key) {
				bucket.credit -= Self::cost(rule, request.tokens);
			}
			if rule.metric == GatewayRuleMetric::Tokens {
				token_buckets.push(key);
			}
		}
		token_buckets
	}

	fn acquire(&mut self, request: &GatewayAdmissionRequest) -> Option<[Subject; 2]> {
		if request.route_class != GatewayRouteClass::Inference {
			return None;
		}
		let subjects = Self::caps(request).map(|(subject, _)| subject);
		for subject in subjects {
			let count = self.concurrency.entry(subject).or_default();
			*count = count.saturating_add(1);
		}
		Some(subjects)
	}

	fn snapshot(&self, request: &GatewayAdmissionRequest) -> GatewayLimitSnapshot {
		GatewayLimitSnapshot {
			requests: self.binding(request, GatewayRuleMetric::Requests),
			tokens: self.binding(request, GatewayRuleMetric::Tokens),
			concurrency: self.concurrency_snapshot(request),
		}
	}

	fn binding(&self, request: &GatewayAdmissionRequest, metric: GatewayRuleMetric) -> Option<GatewayMetricSnapshot> {
		Self::rules(request)
			.filter(|(_, rule)| rule.metric == metric)
			.filter_map(|(key, rule)| self.buckets.get(&key).map(|bucket| (rule.id, bucket)))
			.min_by(|(left_id, left), (right_id, right)| left.fill_cmp(right).then(left_id.cmp(right_id)))
			.map(|(id, bucket)| bucket.snapshot(id))
	}

	fn concurrency_snapshot(&self, request: &GatewayAdmissionRequest) -> Option<(u32, u32)> {
		if request.route_class != GatewayRouteClass::Inference {
			return None;
		}
		let mut selected: Option<(u32, u32)> = None;
		for (subject, cap) in Self::caps(request) {
			let Some(cap) = cap else {
				continue;
			};
			let remaining = cap.saturating_sub(self.active(&subject));
			if selected.is_none_or(|(limit, left)| u64::from(remaining) * u64::from(limit) < u64::from(left) * u64::from(cap)) {
				selected = Some((cap, remaining));
			}
		}
		selected
	}

	fn control(&mut self, command: Control) {
		match command {
			Control::Release(id) => self.release(id),
			Control::Reconcile(id, actual) => self.reconcile(id, actual),
			#[cfg(test)]
			Control::Inspect(reply) => {
				let _ = reply.send(GatewayStateCounts {
						buckets: self.buckets.len(),
						concurrency: self.concurrency.len(),
						reservations: self.reservations.len(),
					});
			}
		}
	}

	fn release(&mut self, id: Uuid) {
		if let Some(subjects) = self.reservations.get_mut(&id).and_then(|record| record.subjects.take()) {
			for subject in subjects {
				if let Some(count) = self.concurrency.get_mut(&subject) {
					*count = count.saturating_sub(1);
				}
			}
		}
		self.settle(id);
	}

	fn reconcile(&mut self, id: Uuid, actual: u64) {
		if let Some(record) = self.reservations.get_mut(&id).filter(|record| !record.reconciled) {
			let adjustment = (i128::from(record.estimated) - i128::from(actual)) * SCALE;
			for key in &record.buckets {
				if let Some(bucket) = self.buckets.get_mut(key) {
					bucket.refill(Instant::now());
					bucket.credit = bucket.credit.saturating_add(adjustment).min(bucket.full());
				}
			}
			record.reconciled = true;
		}
		self.settle(id);
	}

	fn settle(&mut self, id: Uuid) {
		let finished = self
			.reservations
			.get(&id)
			.is_some_and(|record| record.subjects.is_none() && (record.reconciled || record.buckets.is_empty()));
		if finished {
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
					.any(|key| self.buckets.get(key).is_some_and(|bucket| !bucket.is_full()))
		});
		let retained: HashSet<BucketKey> = self.reservations.values().flat_map(|record| record.buckets.iter().copied()).collect();
		self.buckets
			.retain(|key, bucket| !bucket.is_full() || retained.contains(key) || self.concurrency.get(&key.0).is_some_and(|count| *count > 0));
		self.concurrency.retain(|_, count| *count != 0);
	}
}
