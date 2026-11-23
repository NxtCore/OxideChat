//! Which providers, upstream providers and models may see a knowledge base's content (HEN-22 §7).

use super::{KnowledgeActor, KnowledgeBase, RagError};
use crate::logging::AuditLog;
use crate::types::provider_routing::intersect;
use crate::types::providers::ProviderKind;
use crate::types::{AuditEntry, normalize_allowed_providers};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Most targets one knowledge base's model access may hold.
const MAX_TARGETS: usize = 500;

/// The providers, upstream providers and models a knowledge base allows; all empty means no restriction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelTargets {
	#[cfg_attr(test, ts(type = "string[]"))]
	pub providers: Vec<Uuid>,
	/// Slugs of upstream providers that requests through a routing provider (`OpenRouter`, `Cortecs`) may reach.
	pub upstream_providers: Vec<String>,
	#[cfg_attr(test, ts(type = "string[]"))]
	pub models: Vec<Uuid>,
}

/// The model a request goes to, as far as model access cares.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoutedModel {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub kind: ProviderKind,
}

/// Metadata key of chat requests that carries the chat, so every request of a chat with knowledge excerpts is checked.
pub const KNOWLEDGE_CHAT_METADATA_KEY: &str = "oxide_chat_id";

/// Whether knowledge may reach a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAccess {
	Allowed,
	/// Only through these upstream providers of the model's routing provider.
	Upstream(Vec<String>),
	Denied,
}

impl ModelAccess {
	/// Both decisions at once: a denial wins, upstream lists are intersected and none left is a denial.
	#[must_use]
	pub fn and(self, other: Self) -> Self {
		match (self, other) {
			(Self::Denied, _) | (_, Self::Denied) => Self::Denied,
			(Self::Allowed, other) | (other, Self::Allowed) => other,
			(Self::Upstream(left), Self::Upstream(right)) => {
				let both = intersect(&left, &right);
				if both.is_empty() { Self::Denied } else { Self::Upstream(both) }
			}
		}
	}

	/// Allowed without routing that only the chat middleware can apply; embedding, rerank and image requests need this.
	#[must_use]
	pub const fn is_allowed(&self) -> bool {
		matches!(self, Self::Allowed)
	}
}

/// One stored target: knowledge base, then exactly one of provider, model and upstream provider.
type TargetRow = (Uuid, Option<Uuid>, Option<Uuid>, Option<String>);

impl ModelTargets {
	#[must_use]
	pub fn is_unrestricted(&self) -> bool {
		self.providers.is_empty() && self.upstream_providers.is_empty() && self.models.is_empty()
	}

	/// Whether `model` may see the content: a listed model is allowed, a routing provider with upstream providers listed
	/// is narrowed to them, and otherwise its provider must be listed.
	#[must_use]
	pub fn decide(&self, model: &RoutedModel) -> ModelAccess {
		if self.is_unrestricted() || self.models.contains(&model.id) {
			ModelAccess::Allowed
		} else if model.kind.routes_upstream() && !self.upstream_providers.is_empty() {
			ModelAccess::Upstream(self.upstream_providers.clone())
		} else if self.providers.contains(&model.provider_id) {
			ModelAccess::Allowed
		} else {
			ModelAccess::Denied
		}
	}

	/// What every list allows `model`.
	#[must_use]
	pub fn decide_all<'a>(lists: impl IntoIterator<Item = &'a Self>, model: &RoutedModel) -> ModelAccess {
		lists.into_iter().fold(ModelAccess::Allowed, |access, targets| access.and(targets.decide(model)))
	}

	/// The same targets sorted and deduplicated with normalized slugs.
	///
	/// # Errors
	///
	/// Returns `InvalidModelAccess` for an invalid slug or too many targets.
	pub fn normalized(&self) -> Result<Self, RagError> {
		let mut providers = self.providers.clone();
		providers.sort_unstable();
		providers.dedup();
		let mut models = self.models.clone();
		models.sort_unstable();
		models.dedup();
		let upstream_providers = if self.upstream_providers.is_empty() {
			Vec::new()
		} else {
			normalize_allowed_providers(&self.upstream_providers).ok_or(RagError::InvalidModelAccess)?
		};
		if providers.len() + models.len() + upstream_providers.len() > MAX_TARGETS {
			return Err(RagError::InvalidModelAccess);
		}
		Ok(Self { providers, upstream_providers, models })
	}

	/// The targets of each knowledge base among `ids`; bases without any are left out.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn load(pool: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, Self>, sqlx::Error> {
		let rows: Vec<TargetRow> = sqlx::query_as(
			"SELECT knowledge_base_id, provider_id, model_id, upstream_provider FROM knowledge_base_model_targets WHERE knowledge_base_id = ANY($1)
			 ORDER BY knowledge_base_id, provider_id, model_id, upstream_provider",
		)
		.bind(ids)
		.fetch_all(pool)
		.await?;
		let mut targets: HashMap<Uuid, Self> = HashMap::new();
		for (base, provider, model, upstream) in rows {
			let entry = targets.entry(base).or_default();
			entry.providers.extend(provider);
			entry.models.extend(model);
			entry.upstream_providers.extend(upstream);
		}
		Ok(targets)
	}

	/// The targets of every knowledge base whose excerpts are in the chat, skipping unrestricted ones.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn for_chat(pool: &PgPool, chat_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		let ids: Vec<Uuid> = sqlx::query_scalar("SELECT knowledge_base_id FROM chat_knowledge_sources WHERE chat_id = $1")
			.bind(chat_id)
			.fetch_all(pool)
			.await?;
		if ids.is_empty() {
			return Ok(Vec::new());
		}
		Ok(Self::load(pool, &ids).await?.into_values().collect())
	}

	/// The models among `models` these targets let knowledge reach, upstream-narrowed ones included since chat requests
	/// apply the narrowing; `None` without a restriction.
	#[must_use]
	pub fn allowed_models(&self, models: &[RoutedModel]) -> Option<Vec<Uuid>> {
		(!self.is_unrestricted()).then(|| models.iter().filter(|model| self.decide(model) != ModelAccess::Denied).map(|model| model.id).collect())
	}
}

impl RoutedModel {
	/// The model with its provider and provider kind.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn find(pool: &PgPool, model_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>("SELECT m.id, m.provider_id, p.kind FROM models m JOIN providers p ON p.id = m.provider_id WHERE m.id = $1")
			.bind(model_id)
			.fetch_optional(pool)
			.await
	}

	/// Every enabled model of an enabled provider.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn all_enabled(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>("SELECT m.id, m.provider_id, p.kind FROM models m JOIN providers p ON p.id = m.provider_id WHERE m.is_enabled AND p.is_enabled")
			.fetch_all(pool)
			.await
	}
}

/// A provider that model access can list.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelAccessProvider {
	pub id: Uuid,
	pub name: String,
	pub kind: ProviderKind,
	/// Requests can be narrowed to upstream providers.
	pub routes_upstream: bool,
}

/// A model that model access can list.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelAccessModel {
	pub id: Uuid,
	pub name: String,
	pub provider_id: Uuid,
}

/// `GET/PUT …/knowledge-bases/{id}/model-access`: the stored targets and what can be picked.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModelAccessResponse {
	pub can_edit: bool,
	pub targets: ModelTargets,
	pub providers: Vec<ModelAccessProvider>,
	pub models: Vec<ModelAccessModel>,
	/// Upstream provider slugs known from the routing providers.
	pub upstream_providers: Vec<String>,
}

impl KnowledgeBase {
	/// The knowledge base's model access with every installation provider and model to pick from.
	///
	/// # Errors
	///
	/// Returns a database error when a query fails.
	pub async fn model_access(&self, pool: &PgPool, actor: &KnowledgeActor) -> Result<ModelAccessResponse, sqlx::Error> {
		let targets = ModelTargets::load(pool, &[self.id]).await?.remove(&self.id).unwrap_or_default();
		let mut providers: Vec<ModelAccessProvider> =
			sqlx::query_as("SELECT id, name, kind, FALSE AS routes_upstream FROM providers WHERE owner_id IS NULL ORDER BY lower(name), id")
				.fetch_all(pool)
				.await?;
		for provider in &mut providers {
			provider.routes_upstream = provider.kind.routes_upstream();
		}
		let models = sqlx::query_as(
			"SELECT m.id, m.display_name AS name, m.provider_id FROM models m JOIN providers p ON p.id = m.provider_id
			 WHERE p.owner_id IS NULL AND (m.is_enabled OR m.id = ANY($1)) ORDER BY lower(m.display_name), m.id",
		)
		.bind(&targets.models)
		.fetch_all(pool)
		.await?;
		let upstream_providers = sqlx::query_scalar("SELECT DISTINCT slug FROM upstream_providers ORDER BY slug").fetch_all(pool).await?;
		Ok(ModelAccessResponse {
			can_edit: actor.can_edit(self),
			targets,
			providers,
			models,
			upstream_providers,
		})
	}

	/// Replaces the model access, audited; it must allow the knowledge base's embedding model.
	///
	/// # Errors
	///
	/// Returns `Forbidden` unless the actor may edit the base, `InvalidModelAccess` for bad or unknown targets,
	/// `EmbeddingModelNotAllowed`, or a database error.
	pub async fn replace_model_access(&self, pool: &PgPool, actor: &KnowledgeActor, targets: &ModelTargets) -> Result<(), RagError> {
		if !actor.can_edit(self) {
			return Err(RagError::Forbidden);
		}
		let targets = targets.normalized()?;
		check_embedding_allowed(pool, &targets, &self.view.embedding.model_id).await?;
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM knowledge_base_model_targets WHERE knowledge_base_id = $1")
			.bind(self.id)
			.execute(&mut *tx)
			.await?;
		sqlx::query("INSERT INTO knowledge_base_model_targets (knowledge_base_id, provider_id) SELECT $1, unnest($2::uuid[])")
		.bind(self.id)
		.bind(&targets.providers)
		.execute(&mut *tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::InvalidModelAccess))?;
		sqlx::query("INSERT INTO knowledge_base_model_targets (knowledge_base_id, model_id) SELECT $1, unnest($2::uuid[])")
			.bind(self.id)
			.bind(&targets.models)
			.execute(&mut *tx)
			.await
			.map_err(|error| RagError::foreign_key_or(error, RagError::InvalidModelAccess))?;
		sqlx::query("INSERT INTO knowledge_base_model_targets (knowledge_base_id, upstream_provider) SELECT $1, unnest($2::text[])")
			.bind(self.id)
			.bind(&targets.upstream_providers)
			.execute(&mut *tx)
			.await?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "knowledge_base_model_access_updated",
				actor_id: &actor.user_id,
				target_type: "knowledge_base",
				target_id: &self.id,
				metadata: serde_json::json!({ "name": self.name, "targets": targets }),
			},
		)
		.await?;
		tx.commit().await?;
		Ok(())
	}

	/// Records that the chat now holds excerpts of these knowledge bases; the list only grows.
	///
	/// # Errors
	///
	/// Returns a database error when the insert fails.
	pub async fn record_chat_sources(pool: &PgPool, chat_id: &Uuid, ids: &[Uuid]) -> Result<(), sqlx::Error> {
		sqlx::query("INSERT INTO chat_knowledge_sources (chat_id, knowledge_base_id) SELECT $1, unnest($2::uuid[]) ON CONFLICT DO NOTHING")
			.bind(chat_id)
			.bind(ids)
			.execute(pool)
			.await?;
		Ok(())
	}
}

/// Whether a request outside the chat middleware (image, embedding) may send the chat's content to `model_id`.
///
/// Upstream narrowing counts as refused, since only chat requests can apply it.
///
/// # Errors
///
/// Returns a database error when a query fails.
pub async fn chat_allows_model(pool: &PgPool, chat_id: &Uuid, model_id: &Uuid) -> Result<bool, sqlx::Error> {
	let targets = ModelTargets::for_chat(pool, chat_id).await?;
	if targets.iter().all(ModelTargets::is_unrestricted) {
		return Ok(true);
	}
	Ok(RoutedModel::find(pool, model_id)
		.await?
		.is_some_and(|model| ModelTargets::decide_all(&targets, &model).is_allowed()))
}

/// Refuses targets that do not allow `embedding_model_id` outright.
///
/// # Errors
///
/// Returns `EmbeddingModelNotAllowed`, `ModelUnavailable` when the model is gone, or a database error.
pub async fn check_embedding_allowed(pool: &PgPool, targets: &ModelTargets, embedding_model_id: &Uuid) -> Result<(), RagError> {
	if targets.is_unrestricted() {
		return Ok(());
	}
	let model = RoutedModel::find(pool, embedding_model_id).await?.ok_or(RagError::ModelUnavailable)?;
	if targets.decide(&model).is_allowed() { Ok(()) } else { Err(RagError::EmbeddingModelNotAllowed) }
}
