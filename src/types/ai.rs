//! AI Provider types for OxideChat API.
//!
//! Types for managing AI providers, models, and usage tracking.

use crate::types::models_configs::ModelConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Re-export omniference types for convenience
use crate::types::providers::ProviderKind;
pub use omniference::types::ProviderKind as OmniProviderKind;

/// AI Provider database row
#[derive(Debug, Clone, FromRow)]
pub struct AiProvider {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub api_key: Option<String>,
	pub extra_headers: serde_json::Value,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// AI Model database row
#[derive(Debug, Clone, FromRow)]
pub struct AiModel {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: serde_json::Value,
	pub input_modalities: serde_json::Value,
	pub output_modalities: serde_json::Value,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// AI Usage tracking database row
#[derive(Debug, Clone, FromRow)]
pub struct AiUsage {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub provider_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub request_type: String,
	pub input_tokens: Option<i32>,
	pub output_tokens: Option<i32>,
	pub total_tokens: Option<i32>,
	pub latency_ms: Option<i32>,
	pub success: Option<bool>,
	pub error_message: Option<String>,
	pub metadata: serde_json::Value,
	pub created_at: DateTime<Utc>,
}

// ============= Request DTOs =============

/// Request to create a new AI provider
#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub api_key: Option<String>,
	#[serde(default)]
	pub extra_headers: serde_json::Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
}

fn default_true() -> bool {
	true
}

/// Request to update an existing AI provider
#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
	pub kind: Option<ProviderKind>,
	pub name: Option<String>,
	pub base_url: Option<String>,
	pub api_key: Option<String>,
	pub extra_headers: Option<serde_json::Value>,
	pub is_enabled: Option<bool>,
}

/// Request to test provider connection
#[derive(Debug, Deserialize)]
pub struct TestProviderRequest {
	pub kind: ProviderKind,
	pub base_url: String,
	pub api_key: Option<String>,
	#[serde(default)]
	pub extra_headers: serde_json::Value,
}

// ============= Response DTOs =============

/// Provider response (hides sensitive data)
#[derive(Debug, Serialize)]
pub struct ProviderResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub has_api_key: bool,
	pub extra_headers: serde_json::Value,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl From<AiProvider> for ProviderResponse {
	fn from(p: AiProvider) -> Self {
		Self {
			id: p.id,
			owner_id: p.owner_id,
			kind: p.kind,
			name: p.name,
			base_url: p.base_url,
			has_api_key: p.api_key.is_some(),
			extra_headers: p.extra_headers,
			is_enabled: p.is_enabled,
			created_at: p.created_at,
			updated_at: p.updated_at,
		}
	}
}

/// Model response
#[derive(Debug, Serialize)]
pub struct ModelResponse {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: serde_json::Value,
	pub input_modalities: serde_json::Value,
	pub output_modalities: serde_json::Value,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,
	pub is_enabled: bool,
}

/// Model response for the frontend (with flattened provider info)
#[derive(Debug, Serialize)]
pub struct PublicModelResponse {
	pub id: String,
	pub provider_id: String,
	pub model_id: String,
	pub display_name: String,
	pub capabilities: Vec<String>,
	pub input_modalities: Vec<String>,
	pub output_modalities: Vec<String>,
	pub context_length: Option<u32>,
	pub max_tokens: Option<u32>,
	pub provider_name: String,
	pub provider_kind: String,
	pub icon: Option<String>,
}

impl From<AiModel> for ModelResponse {
	fn from(m: AiModel) -> Self {
		Self {
			id: m.id,
			provider_id: m.provider_id,
			model_id: m.model_id,
			display_name: m.display_name,
			capabilities: m.capabilities,
			input_modalities: m.input_modalities,
			output_modalities: m.output_modalities,
			context_length: m.context_length,
			max_tokens: m.max_tokens,
			is_enabled: m.is_enabled,
		}
	}
}

/// Provider test result
#[derive(Debug, Serialize)]
pub struct TestProviderResponse {
	pub success: bool,
	pub models_found: usize,
	pub message: String,
}

/// Provider sync result
#[derive(Debug, Serialize)]
pub struct SyncProviderResponse {
	pub success: bool,
	pub models_added: usize,
	pub models_updated: usize,
	pub models_removed: usize,
	pub message: String,
}

// ============= Provider Metadata (Icons, Display Names) =============

/// Provider metadata for display purposes (icons, colors, etc.)
#[derive(Debug, Clone, FromRow)]
pub struct ProviderMetadata {
	pub id: Uuid,
	pub provider_kind: Option<ProviderKind>,
	pub name_pattern: Option<String>,
	pub display_name: String,
	pub icon_url: Option<String>,
	pub icon_svg: Option<String>,
	pub brand_color: Option<String>,
	pub website_url: Option<String>,
	pub documentation_url: Option<String>,
	pub is_builtin: bool,
	pub priority: i32,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Provider metadata response
#[derive(Debug, Serialize)]
pub struct ProviderMetadataResponse {
	pub id: Uuid,
	pub provider_kind: Option<ProviderKind>,
	pub name_pattern: Option<String>,
	pub display_name: String,
	pub icon_url: Option<String>,
	pub icon_svg: Option<String>,
	pub brand_color: Option<String>,
	pub website_url: Option<String>,
	pub is_builtin: bool,
}

impl From<ProviderMetadata> for ProviderMetadataResponse {
	fn from(m: ProviderMetadata) -> Self {
		Self {
			id: m.id,
			provider_kind: m.provider_kind,
			name_pattern: m.name_pattern,
			display_name: m.display_name,
			icon_url: m.icon_url,
			icon_svg: m.icon_svg,
			brand_color: m.brand_color,
			website_url: m.website_url,
			is_builtin: m.is_builtin,
		}
	}
}

// ============= Model Configurations (User preferences) =============

/// Stable model key format: "provider_kind:model_id"
/// Example: "openai:gpt-4o", "anthropic:claude-3-5-sonnet"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableModelKey(pub String);

impl StableModelKey {
	/// Create a stable key from provider kind and model ID
	#[must_use]
	pub fn new(kind: &ProviderKind, model_id: &str) -> Self {
		let kind_str = kind.as_str();
		Self(format!("{}:{}", kind_str, model_id))
	}

	/// Parse a stable key into (kind, model_id)
	#[must_use]
	pub fn parse(&self) -> Option<(String, String)> {
		let parts: Vec<&str> = self.0.splitn(2, ':').collect();
		if parts.len() == 2 {
			Some((parts[0].to_string(), parts[1].to_string()))
		} else {
			None
		}
	}
}

/// Request to update model config
#[derive(Debug, Deserialize)]
pub struct UpdateModelConfigRequest {
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon_override: Option<String>,
	pub default_temperature: Option<f32>,
	pub default_max_tokens: Option<i32>,
	pub default_top_p: Option<f32>,
	pub default_frequency_penalty: Option<f32>,
	pub default_presence_penalty: Option<f32>,
	pub is_favorite: Option<bool>,
	pub extra_settings: Option<serde_json::Value>,
}

// ============= Agents (Reusable AI configurations) =============

/// Agent database row
#[derive(Debug, Clone, FromRow)]
pub struct Agent {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub slug: Option<String>,
	pub description: Option<String>,
	pub icon_url: Option<String>,
	pub icon_emoji: Option<String>,
	pub model_stable_key: Option<String>,
	pub fallback_model_keys: serde_json::Value,
	pub system_prompt: Option<String>,
	pub initial_messages: serde_json::Value,
	pub temperature: Option<f32>,
	pub max_tokens: Option<i32>,
	pub top_p: Option<f32>,
	pub frequency_penalty: Option<f32>,
	pub presence_penalty: Option<f32>,
	pub enabled_tools: serde_json::Value,
	pub web_search_enabled: bool,
	pub code_execution_enabled: bool,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub category: Option<String>,
	pub tags: serde_json::Value,
	pub usage_count: i32,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Agent response
#[derive(Debug, Serialize)]
pub struct AgentResponse {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub name: String,
	pub slug: Option<String>,
	pub description: Option<String>,
	pub icon_url: Option<String>,
	pub icon_emoji: Option<String>,
	pub model_stable_key: Option<String>,
	pub fallback_model_keys: Vec<String>,
	pub system_prompt: Option<String>,
	pub temperature: Option<f32>,
	pub max_tokens: Option<i32>,
	pub enabled_tools: Vec<String>,
	pub web_search_enabled: bool,
	pub code_execution_enabled: bool,
	pub is_public: bool,
	pub is_featured: bool,
	pub is_default: bool,
	pub category: Option<String>,
	pub tags: Vec<String>,
	pub usage_count: i32,
	pub created_at: DateTime<Utc>,
}

impl From<Agent> for AgentResponse {
	fn from(a: Agent) -> Self {
		Self {
			id: a.id,
			owner_id: a.owner_id,
			name: a.name,
			slug: a.slug,
			description: a.description,
			icon_url: a.icon_url,
			icon_emoji: a.icon_emoji,
			model_stable_key: a.model_stable_key,
			fallback_model_keys: serde_json::from_value(a.fallback_model_keys).unwrap_or_default(),
			system_prompt: a.system_prompt,
			temperature: a.temperature,
			max_tokens: a.max_tokens,
			enabled_tools: serde_json::from_value(a.enabled_tools).unwrap_or_default(),
			web_search_enabled: a.web_search_enabled,
			code_execution_enabled: a.code_execution_enabled,
			is_public: a.is_public,
			is_featured: a.is_featured,
			is_default: a.is_default,
			category: a.category,
			tags: serde_json::from_value(a.tags).unwrap_or_default(),
			usage_count: a.usage_count,
			created_at: a.created_at,
		}
	}
}

/// Request to create an agent
#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
	pub name: String,
	pub slug: Option<String>,
	pub description: Option<String>,
	pub icon_url: Option<String>,
	pub icon_emoji: Option<String>,
	pub model_stable_key: Option<String>,
	#[serde(default)]
	pub fallback_model_keys: Vec<String>,
	pub system_prompt: Option<String>,
	#[serde(default)]
	pub initial_messages: Vec<serde_json::Value>,
	pub temperature: Option<f32>,
	pub max_tokens: Option<i32>,
	pub top_p: Option<f32>,
	#[serde(default)]
	pub enabled_tools: Vec<String>,
	#[serde(default)]
	pub web_search_enabled: bool,
	#[serde(default)]
	pub code_execution_enabled: bool,
	#[serde(default)]
	pub is_public: bool,
	pub category: Option<String>,
	#[serde(default)]
	pub tags: Vec<String>,
}

/// Request to update an agent
#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
	pub name: Option<String>,
	pub slug: Option<String>,
	pub description: Option<String>,
	pub icon_url: Option<String>,
	pub icon_emoji: Option<String>,
	pub model_stable_key: Option<String>,
	pub fallback_model_keys: Option<Vec<String>>,
	pub system_prompt: Option<String>,
	pub initial_messages: Option<Vec<serde_json::Value>>,
	pub temperature: Option<f32>,
	pub max_tokens: Option<i32>,
	pub top_p: Option<f32>,
	pub enabled_tools: Option<Vec<String>>,
	pub web_search_enabled: Option<bool>,
	pub code_execution_enabled: Option<bool>,
	pub is_public: Option<bool>,
	pub is_featured: Option<bool>,
	pub is_default: Option<bool>,
	pub category: Option<String>,
	pub tags: Option<Vec<String>>,
}

// ============= Combined Model Info (with metadata) =============

/// A model with resolved metadata and user config
#[derive(Debug, Serialize)]
pub struct ModelWithMetadata {
	pub id: Uuid,
	pub provider_id: Uuid,
	pub model_id: String,
	pub display_name: String,
	pub stable_key: String,
	pub capabilities: Vec<ModelCapabilities>,
	pub context_length: Option<i32>,
	pub max_tokens: Option<i32>,

	// From provider
	pub provider_name: String,
	pub provider_kind: ProviderKind,

	pub provider_display_name: String,
	pub provider_icon_svg: Option<String>,
	pub provider_brand_color: Option<String>,

	// From model_configs (user preferences, if any)
	pub user_display_name: Option<String>,
	pub user_icon_override: Option<String>,
	pub is_favorite: bool,
	pub default_temperature: Option<f32>,
	pub default_max_tokens: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelCapabilities {
	ReasoningEffortNone,
	ReasoningEffortMinimal,
	ReasoningEffortLow,
	ReasoningEffortMedium,
	ReasoningEffortHigh,
	ReasoningEffortXHigh,
	Tools,
}

impl ModelCapabilities {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::ReasoningEffortNone => "REASONING_EFFORT_NONE",
			Self::ReasoningEffortMinimal => "REASONING_EFFORT_MINIMAL",
			Self::ReasoningEffortLow => "REASONING_EFFORT_LOW",
			Self::ReasoningEffortMedium => "REASONING_EFFORT_MEDIUM",
			Self::ReasoningEffortHigh => "REASONING_EFFORT_HIGH",
			Self::ReasoningEffortXHigh => "REASONING_EFFORT_XHIGH",
			Self::Tools => "TOOLS",
		}
	}
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"REASONING_EFFORT_NONE" => Some(Self::ReasoningEffortNone),
			"REASONING_EFFORT_MINIMAL" => Some(Self::ReasoningEffortMinimal),
			"REASONING_EFFORT_LOW" => Some(Self::ReasoningEffortLow),
			"REASONING_EFFORT_MEDIUM" => Some(Self::ReasoningEffortMedium),
			"REASONING_EFFORT_HIGH" => Some(Self::ReasoningEffortHigh),
			"REASONING_EFFORT_XHIGH" => Some(Self::ReasoningEffortXHigh),
			"TOOLS" => Some(Self::Tools),
			_ => None,
		}
	}
}

impl crate::types::BaseType for AiProvider {
	const TABLE: &'static str = "providers";
	const ALIAS: &'static str = "p";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			owner_id: None,
			kind: ProviderKind::Openai,
			name: String::new(),
			base_url: String::new(),
			api_key: None,
			extra_headers: serde_json::Value::Null,
			is_enabled: true,
			created_at: Utc::now(),
			updated_at: Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id",
			"owner_id",
			"kind",
			"name",
			"base_url",
			"api_key",
			"extra_headers",
			"is_enabled",
			"created_at",
			"updated_at",
		]
	}
}

impl AiProvider {
	pub async fn list_system(pool: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, AiProvider>("SELECT * FROM providers WHERE owner_id IS NULL ORDER BY name")
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_id_system(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, AiProvider>("SELECT * FROM providers WHERE id = $1 AND owner_id IS NULL")
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(
		pool: &sqlx::PgPool,
		owner_id: Option<&Uuid>,
		kind: &ProviderKind,
		name: &str,
		base_url: &str,
		api_key: Option<&str>,
		extra_headers: Option<&serde_json::Value>,
		is_enabled: bool,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, AiProvider>(
			r#"
			INSERT INTO providers (owner_id, kind, name, base_url, api_key, extra_headers, is_enabled)
			VALUES ($1, $2, $3, $4, $5, $6, $7)
			RETURNING *
			"#,
		)
		.bind(owner_id)
		.bind(kind)
		.bind(name)
		.bind(base_url)
		.bind(api_key)
		.bind(extra_headers)
		.bind(is_enabled)
		.fetch_one(pool)
		.await
	}

	pub async fn update(
		pool: &sqlx::PgPool,
		id: &Uuid,
		kind: Option<&ProviderKind>,
		name: Option<&str>,
		base_url: Option<&str>,
		api_key: Option<&str>,
		extra_headers: Option<&serde_json::Value>,
		is_enabled: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, AiProvider>(
			r#"
			UPDATE providers
			SET kind = COALESCE($2, kind),
			    name = COALESCE($3, name),
			    base_url = COALESCE($4, base_url),
			    api_key = COALESCE($5, api_key),
			    extra_headers = COALESCE($6, extra_headers),
			    is_enabled = COALESCE($7, is_enabled),
			    updated_at = NOW()
			WHERE id = $1
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(kind)
		.bind(name)
		.bind(base_url)
		.bind(api_key)
		.bind(extra_headers)
		.bind(is_enabled)
		.fetch_optional(pool)
		.await
	}

	pub async fn delete(pool: &sqlx::PgPool, id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM providers WHERE id = $1").bind(id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}
}

impl AiModel {
	pub async fn list_by_provider(pool: &sqlx::PgPool, provider_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, AiModel>("SELECT * FROM models WHERE provider_id = $1 ORDER BY display_name")
			.bind(provider_id)
			.fetch_all(pool)
			.await
	}
}

impl crate::types::BaseType for AiUsage {
	const TABLE: &'static str = "usage";
	const ALIAS: &'static str = "u";

	fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			user_id: None,
			provider_id: None,
			model_id: None,
			request_type: String::new(),
			input_tokens: None,
			output_tokens: None,
			total_tokens: None,
			latency_ms: None,
			success: None,
			error_message: None,
			metadata: serde_json::Value::Null,
			created_at: Utc::now(),
		}
	}

	fn sql_fields() -> &'static [&'static str] {
		&[
			"id",
			"user_id",
			"provider_id",
			"model_id",
			"request_type",
			"input_tokens",
			"output_tokens",
			"total_tokens",
			"latency_ms",
			"success",
			"error_message",
			"metadata",
			"created_at",
		]
	}
}
