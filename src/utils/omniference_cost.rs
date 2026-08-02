use crate::types::models::{Model, ModelPricing};
use crate::types::{Budget, JobState, UsageEvent, UsageEventRecord};
use async_trait::async_trait;
use omniference::middleware::cost::{AsyncCostSink, CostRecord};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::sync::Arc;
use uuid::Uuid;

pub struct OxideCostSink {
	state: Arc<JobState>,
}

impl OxideCostSink {
	#[must_use]
	pub fn new(state: Arc<JobState>) -> Self {
		Self { state }
	}
}

#[async_trait]
impl AsyncCostSink for OxideCostSink {
	async fn record(&self, record: CostRecord) {
		if let Err(error) = self.persist(&record).await {
			tracing::error!(%error, provider = record.provider, model = record.model, "failed to persist Omniference usage");
		}
	}
}

impl OxideCostSink {
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
		UsageEvent::record(
			&self.state.db,
			UsageEventRecord {
				user_id: &user_id,
				team_id,
				model_id: &model.id,
				provider_id: &model.provider_id,
				request_type: "gateway",
				input_tokens,
				output_tokens,
				reasoning_tokens,
				cost_total,
			},
		)
		.await?;
		Ok(())
	}
}

fn metadata_uuid(record: &CostRecord, key: &str) -> Option<Uuid> {
	record.metadata.get(key).and_then(|value| Uuid::parse_str(value).ok())
}

fn token_count(tokens: u32) -> i32 {
	tokens.min(i32::MAX as u32) as i32
}
