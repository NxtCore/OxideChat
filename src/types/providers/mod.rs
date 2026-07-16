use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

mod billing;
mod billing_repository;
mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub(crate) use billing::{ProviderBillingConnection, ProviderBillingMetric, ProviderBillingOverviewRow};
pub use omniference::types::ProviderKind as OmniProviderKind;
pub use requests::*;
pub use responses::*;

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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
	pub id: Uuid,
	pub owner_id: Option<Uuid>,
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub api_key: Option<String>,
	pub extra_headers: Json<Value>,
	pub is_enabled: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProviderSlim {
	pub id: Uuid,
	pub name: String,
	pub kind: ProviderKind,
}

impl BaseType for Provider {}
