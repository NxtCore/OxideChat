use crate::types::models::{Model, ModelPricing};
use crate::types::{Budget, JobState, UsageEvent, UsageEventRecord};
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

	async fn persist(&self, record: &CostRecord) -> Result<(), sqlx::Error> {
		let Some(user_id) = metadata_uuid(record, "oxide_user_id") else {
			return Ok(());
		};
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
		let team_id = match metadata_uuid(record, "oxide_team_id") {
			Some(team_id) => Some(team_id),
			None => Budget::primary_team_id(&self.state.db, &user_id).await?,
		};
		let usage = UsageEventRecord {
			user_id: &user_id,
			team_id,
			model_id: &model.id,
			provider_id: &model.provider_id,
			request_type: "gateway",
			input_tokens,
			output_tokens,
			reasoning_tokens,
			cost_total,
		};
		UsageEvent::record(&self.state.db, usage).await?;
		Ok(())
	}
}

/// Managed asynchronous cost queue with retry and shutdown handling.
pub struct OxideCostQueue {
	sender: mpsc::UnboundedSender<CostQueueMessage>,
	shutdown_started: AtomicBool,
	shutdown_complete: AtomicBool,
	shutdown_notify: Notify,
}

impl OxideCostQueue {
	/// Starts a cost queue that writes through the supplied application state.
	#[must_use]
	pub fn spawn(state: Arc<JobState>) -> Arc<Self> {
		let (sender, mut receiver) = mpsc::unbounded_channel();
		let sink = OxideCostSink::new(state);
		tokio::spawn(async move {
			while let Some(message) = receiver.recv().await {
				match message {
					CostQueueMessage::Record(record) => {
						let mut attempt = 0_u32;
						loop {
							match sink.persist(&record).await {
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
		if self.sender.send(CostQueueMessage::Record(record)).is_err() {
			tracing::error!(provider, model, "asynchronous cost sink stopped before cost could be recorded");
		}
	}
}

impl CostSink for OxideCostQueue {
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
	Record(CostRecord),
	Shutdown(oneshot::Sender<()>),
}

fn metadata_uuid(record: &CostRecord, key: &str) -> Option<Uuid> {
	record.metadata.get(key).and_then(|value| Uuid::parse_str(value).ok())
}

fn token_count(tokens: u32) -> i32 {
	tokens.min(i32::MAX as u32) as i32
}
