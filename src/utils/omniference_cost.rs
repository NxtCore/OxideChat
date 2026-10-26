use crate::types::models::{Model, ModelPricing};
use crate::types::{
	BUDGET_RESERVATION_METADATA_KEY, Budget, BudgetLedger, GatewayLimitBackend, GatewayLimiter, JobState, PUBLIC_MODEL_METADATA_KEY, REQUEST_ID_METADATA_KEY, RESERVATION_METADATA_KEY,
	RequestLog, RequestLogContext, RequestLogErrorCategory, RequestLogRecord, RequestLogStatus, STARTED_AT_METADATA_KEY, STREAMING_METADATA_KEY, UsageEvent, UsageEventRecord,
};
use chrono::{DateTime, Utc};
use omniference::catalog::UsageBreakdown;
use omniference::middleware::cost::{CostFinalization, CostRecord, CostSink};
use omniference::stream::CostDetails;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::collections::BTreeMap;
use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, mpsc, oneshot};
use uuid::Uuid;

/// Persists gateway usage received from the asynchronous cost queue.
pub struct OxideCostSink {
	state: Arc<JobState>,
}

impl OxideCostSink {
	/// Creates a sink that persists gateway usage through shared application state.
	#[must_use]
	pub fn new(state: Arc<JobState>) -> Self {
		Self { state }
	}

	async fn persist(&self, record: &CostRecord, finished_at: DateTime<Utc>) -> Result<(), sqlx::Error> {
		let budget_id = metadata_uuid(record, BUDGET_RESERVATION_METADATA_KEY);
		self.store(record, budget_id, finished_at).await?;
		if let Some(id) = budget_id {
			self.state.budget_ledger.settle(id);
		}
		Ok(())
	}

	async fn store(&self, record: &CostRecord, budget_id: Option<Uuid>, finished_at: DateTime<Utc>) -> Result<(), sqlx::Error> {
		let user_id = metadata_uuid(record, "oxide_user_id");
		let api_key_id = metadata_uuid(record, "oxide_api_key_id");
		let metadata_team_id = metadata_uuid(record, "oxide_team_id");
		if user_id.is_none() && (metadata_team_id.is_none() || api_key_id.is_none()) {
			return Ok(());
		}
		let Some(model_id) = metadata_uuid(record, "oxide_model_id") else {
			return Ok(());
		};
		let Some(model) = Model::find_by_id(&self.state.db, &model_id).await? else {
			return Ok(());
		};
		let input_tokens = token_count(record.usage.input_tokens);
		let output_tokens = token_count(record.usage.output_tokens);
		let reasoning_tokens = token_count(record.usage.reasoning_tokens);
		let cost_total = ModelPricing::usage_cost(&self.state.db, &model.id, input_tokens, output_tokens, reasoning_tokens)
			.await?
			.unwrap_or_else(|| Decimal::from_f64(record.cost.total).unwrap_or(Decimal::ZERO));
		let team_id = match (metadata_team_id, user_id.as_ref()) {
			(Some(team_id), _) => Some(team_id),
			(None, Some(user_id)) => Budget::primary_team_id(&self.state.db, user_id).await?,
			(None, None) => None,
		};
		if let Some(id) = budget_id {
			self.state.budget_ledger.reconcile(id, cost_total);
		}
		let usage = UsageEventRecord {
			user_id: user_id.as_ref(),
			team_id,
			api_key_id,
			model_id: &model.id,
			provider_id: &model.provider_id,
			request_type: record.metadata.get("oxide_request_type").map_or("gateway", String::as_str),
			input_tokens,
			output_tokens,
			reasoning_tokens,
			cost_total,
		};
		UsageEvent::record(&self.state.db, usage).await?;
		let (status, error_category) = request_outcome(record.finalization);
		RequestLog::record(
			&self.state.db,
			RequestLogRecord {
				user_id,
				team_id,
				api_key_id,
				model_id: Some(model.id),
				public_model_id: record.metadata.get(PUBLIC_MODEL_METADATA_KEY).map_or(model.model_id.as_str(), String::as_str),
				request_type: record.metadata.get("oxide_request_type").map_or("gateway", String::as_str),
				status,
				status_code: None,
				input_tokens,
				output_tokens,
				reasoning_tokens,
				cost_total,
				request_id: metadata_uuid(record, REQUEST_ID_METADATA_KEY),
				provider: Some(record.provider.as_str()),
				error_category,
				streaming: record.metadata.get(STREAMING_METADATA_KEY).is_some_and(|value| value == "true"),
				latency_ms: latency_ms(record, finished_at),
			},
		)
		.await?;
		Ok(())
	}
}

/// Managed asynchronous cost queue with retry and shutdown handling.
pub struct OxideCostQueue {
	limiter: GatewayLimiter,
	ledger: BudgetLedger,
	sender: mpsc::UnboundedSender<CostQueueMessage>,
	shutdown_started: AtomicBool,
	shutdown_complete: AtomicBool,
	shutdown_notify: Notify,
}

impl OxideCostQueue {
	/// Starts a cost queue that writes through the supplied application state.
	#[must_use]
	pub fn spawn(state: Arc<JobState>) -> Arc<Self> {
		let limiter = state.gateway_limiter.clone();
		let ledger = state.budget_ledger.clone();
		let (sender, mut receiver) = mpsc::unbounded_channel();
		let sink = OxideCostSink::new(state);
		tokio::spawn(async move {
			while let Some(message) = receiver.recv().await {
				match message {
					CostQueueMessage::Record(record, finished_at) => {
						let mut attempt = 0_u32;
						loop {
							match sink.persist(&record, finished_at).await {
								Ok(()) => break,
								Err(error) if attempt < 2 => {
									attempt += 1;
									tracing::warn!(%error, attempt, provider = record.provider, model = record.model, "retrying Omniference usage persistence");
									tokio::time::sleep(std::time::Duration::from_millis(100 * u64::from(attempt))).await;
								}
								Err(error) => {
									tracing::error!(%error, provider = record.provider, model = record.model, "failed to persist Omniference usage");
									break;
								}
							}
						}
					}
					CostQueueMessage::Shutdown(completion) => {
						let _ = completion.send(());
						break;
					}
				}
			}
		});
		Arc::new(Self {
			limiter,
			ledger,
			sender,
			shutdown_started: AtomicBool::new(false),
			shutdown_complete: AtomicBool::new(false),
			shutdown_notify: Notify::new(),
		})
	}

	/// Drains all queued cost operations and awaits the worker.
	pub async fn shutdown(&self) {
		if !self.shutdown_started.swap(true, Ordering::AcqRel) {
			let (completion, completed) = oneshot::channel();
			if self.sender.send(CostQueueMessage::Shutdown(completion)).is_err() {
				tracing::error!("cost queue worker stopped before shutdown");
			}
			if completed.await.is_err() {
				tracing::error!("cost queue worker failed during shutdown");
			}
			self.shutdown_complete.store(true, Ordering::Release);
			self.shutdown_notify.notify_waiters();
			return;
		}
		while !self.shutdown_complete.load(Ordering::Acquire) {
			let notified = self.shutdown_notify.notified();
			if self.shutdown_complete.load(Ordering::Acquire) {
				break;
			}
			notified.await;
		}
	}

	fn queue_record(&self, record: CostRecord) {
		let provider = record.provider.clone();
		let model = record.model.clone();
		if self.sender.send(CostQueueMessage::Record(record, Utc::now())).is_err() {
			tracing::error!(provider, model, "asynchronous cost sink stopped before cost could be recorded");
		}
	}
}

impl CostSink for OxideCostQueue {
	fn terminal_usage(&self, _finalization: CostFinalization, metadata: &BTreeMap<String, String>, usage: &UsageBreakdown) {
		if let Some(id) = metadata.get(BUDGET_RESERVATION_METADATA_KEY).and_then(|value| Uuid::parse_str(value).ok()) {
			self.ledger.finalize(id);
		}
		if let Some(id) = metadata.get(RESERVATION_METADATA_KEY).and_then(|value| Uuid::parse_str(value).ok()) {
			self.limiter
				.reconcile(id, u64::from(usage.input_tokens) + u64::from(usage.output_tokens.max(usage.reasoning_tokens)));
		}
	}

	fn record(&self, provider: &str, model: &str, cost: &CostDetails, finalization: CostFinalization) {
		self.record_with_context(provider, model, cost, finalization, &BTreeMap::new(), &UsageBreakdown::default());
	}

	fn record_with_context(
		&self,
		provider: &str,
		model: &str,
		cost: &CostDetails,
		finalization: CostFinalization,
		metadata: &BTreeMap<String, String>,
		usage: &UsageBreakdown,
	) {
		self.queue_record(CostRecord {
			provider: provider.to_string(),
			model: model.to_string(),
			cost: cost.clone(),
			finalization,
			metadata: metadata.clone(),
			usage: usage.clone(),
		});
	}

	fn record_usage(&self, provider: &str, model: &str, finalization: CostFinalization, metadata: &BTreeMap<String, String>, usage: &UsageBreakdown) {
		self.record_with_context(
			provider,
			model,
			&CostDetails {
				total: 0.0,
				prompt: None,
				completion: None,
				reasoning: None,
			},
			finalization,
			metadata,
			usage,
		);
	}
}

enum CostQueueMessage {
	Record(CostRecord, DateTime<Utc>),
	Shutdown(oneshot::Sender<()>),
}

fn metadata_uuid(record: &CostRecord, key: &str) -> Option<Uuid> {
	record.metadata.get(key).and_then(|value| Uuid::parse_str(value).ok())
}

fn token_count(tokens: u32) -> i32 {
	tokens.min(i32::MAX as u32) as i32
}

fn request_outcome(finalization: CostFinalization) -> (RequestLogStatus, Option<RequestLogErrorCategory>) {
	match finalization {
		CostFinalization::ProviderReported | CostFinalization::Done => (RequestLogStatus::Success, None),
		CostFinalization::Error => (RequestLogStatus::Error, Some(RequestLogErrorCategory::Upstream)),
		CostFinalization::EndOfStream | CostFinalization::Dropped => (RequestLogStatus::Error, Some(RequestLogErrorCategory::Interrupted)),
	}
}

fn latency_ms(record: &CostRecord, finished_at: DateTime<Utc>) -> Option<i32> {
	let started_at = record.metadata.get(STARTED_AT_METADATA_KEY).and_then(|value| RequestLogContext::parse_started_at(value))?;
	Some(RequestLogContext::latency_between(started_at, finished_at))
}
