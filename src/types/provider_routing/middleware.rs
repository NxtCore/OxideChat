use super::{EffectiveProviderRouting, ProviderRoutingError};
use crate::i18n::I18n;
use crate::types::JobState;
use crate::types::models::Model;
use crate::types::providers::ProviderKind;
use async_trait::async_trait;
use omniference::adapter::AdapterError;
use omniference::middleware::{ChatStream, Middleware, RequestHandler};
use omniference::types::{ChatRequestIR, ProviderRouting};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Restricts every request to a routing provider kind to the upstream providers its subject may use.
///
/// The subject comes from the request's `oxide_*` metadata: a gateway request carries the key's team,
/// web chat carries the user, whose teams are combined.
pub struct ProviderRoutingMiddleware {
	state: Arc<JobState>,
}

impl ProviderRoutingMiddleware {
	#[must_use]
	pub fn new(state: Arc<JobState>) -> Self {
		Self { state }
	}

	async fn routing_for(&self, request: &ChatRequestIR) -> anyhow::Result<Option<ProviderRouting>> {
		let current = request.provider_routing.clone();
		let Some(model_id) = metadata_uuid(&request.metadata, "oxide_model_id") else {
			return Ok(current);
		};
		let pool = &self.state.db;
		let Some(kind) = Model::provider_kind(pool, &model_id).await?.filter(ProviderKind::routes_upstream) else {
			return Ok(current);
		};
		let policy = match (metadata_uuid(&request.metadata, "oxide_team_id"), metadata_uuid(&request.metadata, "oxide_user_id")) {
			(Some(team_id), _) => EffectiveProviderRouting::for_team(pool, &team_id).await?,
			(None, Some(user_id)) => EffectiveProviderRouting::for_user(pool, &user_id).await?,
			(None, None) => EffectiveProviderRouting::installation(),
		};
		policy.restrict(&kind, current).map_err(|error| not_allowed(&error).into())
	}
}

#[async_trait]
impl Middleware for ProviderRoutingMiddleware {
	async fn handle(&self, mut request: ChatRequestIR, cancel: CancellationToken, next: &dyn RequestHandler) -> anyhow::Result<ChatStream> {
		request.provider_routing = self.routing_for(&request).await?;
		next.handle(request, cancel).await
	}
}

fn metadata_uuid(metadata: &BTreeMap<String, String>, key: &str) -> Option<Uuid> {
	metadata.get(key).and_then(|value| Uuid::parse_str(value).ok())
}

/// The error a refused request is answered with: an HTTP 403 in the `OpenAI` error format.
fn not_allowed(error: &ProviderRoutingError) -> AdapterError {
	let key = match error {
		ProviderRoutingError::NotAllowed(_) => "chat.errors.provider_not_allowed",
		ProviderRoutingError::NoneAllowed => "chat.errors.no_provider_allowed",
	};
	AdapterError::Forbidden {
		code: "provider_not_allowed".to_string(),
		message: I18n::get().translate(key, &None),
	}
}
