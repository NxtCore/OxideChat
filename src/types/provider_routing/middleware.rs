use super::{EffectiveProviderRouting, ProviderRoutingError};
use crate::i18n::I18n;
use crate::types::JobState;
use crate::types::models::Model;
use crate::types::providers::ProviderKind;
use crate::types::rag::{KNOWLEDGE_CHAT_METADATA_KEY, ModelAccess, ModelTargets, RoutedModel};
use async_trait::async_trait;
use omniference::adapter::AdapterError;
use omniference::middleware::{ChatStream, Middleware, RequestHandler};
use omniference::types::{ChatRequestIR, ProviderRouting};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Restricts every request to a routing provider kind to the upstream providers its subject may use, and every request
/// of a chat holding knowledge excerpts to the models and providers those knowledge bases allow.
///
/// The subject comes from the request's `oxide_*` metadata: a gateway request carries the key's team,
/// web chat carries the user, whose teams are combined, and its chat.
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

	/// Refuses a model the chat's knowledge bases don't allow, and narrows routing providers to their upstream providers.
	async fn knowledge_routing(&self, request: &ChatRequestIR) -> anyhow::Result<Option<ProviderRouting>> {
		let current = request.provider_routing.clone();
		let Some(chat_id) = metadata_uuid(&request.metadata, KNOWLEDGE_CHAT_METADATA_KEY) else {
			return Ok(current);
		};
		let pool = &self.state.db;
		let targets = ModelTargets::for_chat(pool, &chat_id).await?;
		if targets.iter().all(ModelTargets::is_unrestricted) {
			return Ok(current);
		}
		let model = match metadata_uuid(&request.metadata, "oxide_model_id") {
			Some(model_id) => RoutedModel::find(pool, &model_id).await?,
			None => None,
		};
		let Some(model) = model else {
			return Err(knowledge_not_allowed().into());
		};
		match ModelTargets::decide_all(&targets, &model) {
			ModelAccess::Allowed => Ok(current),
			ModelAccess::Denied => Err(knowledge_not_allowed().into()),
			ModelAccess::Upstream(allowed) => narrow(current, &allowed).ok_or_else(|| knowledge_not_allowed().into()),
		}
	}
}

/// Keeps a routing to the `allowed` upstream providers; `None` when the request names only others.
pub(crate) fn narrow(routing: Option<ProviderRouting>, allowed: &[String]) -> Option<Option<ProviderRouting>> {
	let mut routing = routing.unwrap_or_default();
	let only = match routing.only.take().filter(|only| !only.is_empty()) {
		Some(only) => super::intersect(&only, allowed),
		None => allowed.to_vec(),
	};
	if only.is_empty() {
		return None;
	}
	if let Some(order) = routing.order.as_mut() {
		order.retain(|slug| !super::intersect(std::slice::from_ref(slug), &only).is_empty());
	}
	routing.only = Some(only);
	Some(Some(routing))
}

fn knowledge_not_allowed() -> AdapterError {
	AdapterError::Forbidden {
		code: "knowledge_model_not_allowed".to_string(),
		message: I18n::get().translate("chat.errors.knowledge_model_not_allowed", &None),
	}
}

#[async_trait]
impl Middleware for ProviderRoutingMiddleware {
	async fn handle(&self, mut request: ChatRequestIR, cancel: CancellationToken, next: &dyn RequestHandler) -> anyhow::Result<ChatStream> {
		request.provider_routing = self.routing_for(&request).await?;
		request.provider_routing = self.knowledge_routing(&request).await?;
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
