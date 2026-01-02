//! AI Provider types for OxideChat API.
//!
//! Types for managing AI providers, models, and usage tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Re-export omniference types for convenience
pub use omniference::types::ProviderKind as OmniProviderKind;

/// Provider kind enum matching the database enum
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "provider_kind", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderKind {
	Openai,
	OpenaiCompat,
	Openrouter,
	Anthropic,
	Google,
	Custom,
}

impl ProviderKind {
	/// Convert to omniference ProviderKind
	#[must_use]
	pub fn to_omni_kind(&self) -> OmniProviderKind {
		match self {
			Self::Openai => OmniProviderKind::OpenAI,
			Self::OpenaiCompat => OmniProviderKind::OpenAICompat,
			Self::Anthropic => OmniProviderKind::Anthropic,
			Self::Google => OmniProviderKind::Google,
			Self::Openrouter => OmniProviderKind::OpenRouter,
			Self::Custom => OmniProviderKind::Custom("CUSTOM".to_string()),
		}
	}

	/// Convert from omniference ProviderKind
	#[must_use]
	pub fn from_omni_kind(kind: &OmniProviderKind) -> Self {
		match kind {
			OmniProviderKind::OpenAI => Self::Openai,
			OmniProviderKind::OpenAICompat => Self::OpenaiCompat,
			OmniProviderKind::Anthropic => Self::Anthropic,
			OmniProviderKind::Google => Self::Google,
			OmniProviderKind::OpenRouter => Self::Openrouter,
			OmniProviderKind::Custom(_) => Self::Custom,
		}
	}

	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Openai => "OPENAI",
			Self::OpenaiCompat => "OPENAI_COMPAT",
			Self::Openrouter => "OPENROUTER",
			Self::Anthropic => "ANTHROPIC",
			Self::Google => "GOOGLE",
			Self::Custom => "CUSTOM",
		}
	}

	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"OPENAI" => Some(Self::Openai),
			"OPENAI_COMPAT" => Some(Self::OpenaiCompat),
			"OPENROUTER" => Some(Self::Openrouter),
			"ANTHROPIC" => Some(Self::Anthropic),
			"GOOGLE" => Some(Self::Google),
			"CUSTOM" => Some(Self::Custom),
			_ => None,
		}
	}
}

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

/// Model configuration database row (user preferences)
#[derive(Debug, Clone, FromRow)]
pub struct ModelConfig {
	pub id: Uuid,
	pub user_id: Option<Uuid>,
	pub stable_key: String,
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon_override: Option<String>,
	pub default_temperature: Option<f32>,
	pub default_max_tokens: Option<i32>,
	pub default_top_p: Option<f32>,
	pub default_frequency_penalty: Option<f32>,
	pub default_presence_penalty: Option<f32>,
	pub is_favorite: bool,
	pub is_hidden: bool,
	pub extra_settings: serde_json::Value,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Model config response
#[derive(Debug, Serialize)]
pub struct ModelConfigResponse {
	pub id: Uuid,
	pub stable_key: String,
	pub display_name: Option<String>,
	pub description: Option<String>,
	pub icon_override: Option<String>,
	pub default_temperature: Option<f32>,
	pub default_max_tokens: Option<i32>,
	pub default_top_p: Option<f32>,
	pub is_favorite: bool,
	pub is_hidden: bool,
}

impl From<ModelConfig> for ModelConfigResponse {
	fn from(m: ModelConfig) -> Self {
		Self {
			id: m.id,
			stable_key: m.stable_key,
			display_name: m.display_name,
			description: m.description,
			icon_override: m.icon_override,
			default_temperature: m.default_temperature,
			default_max_tokens: m.default_max_tokens,
			default_top_p: m.default_top_p,
			is_favorite: m.is_favorite,
			is_hidden: m.is_hidden,
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
	pub is_hidden: Option<bool>,
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
	pub is_hidden: bool,
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
