use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use crate::types::{BaseType, OmniProviderKind};

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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
    pub id: Uuid,
    pub owner_id: Option<Uuid>,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub extra_headers: Json<serde_json::Value>,
    pub is_enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProviderSlim {
    pub id: Uuid,
    pub name: String,
    pub kind: ProviderKind,
}

impl BaseType for Provider {
    const TABLE: &'static str = "providers";
    const ALIAS: &'static str = "p";

    fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            owner_id: None,
            kind: ProviderKind::Openai,
            name: String::from("OpenAI"),
            base_url: String::from("https://api.openai.com/v1"),
            api_key: None,
            extra_headers: Json(serde_json::Value::Null),
            is_enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
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

impl Provider {
    pub async fn list_paginated(pool: &sqlx::PgPool, page: i64, per_page: i64) -> Result<Vec<Provider>, sqlx::Error> {
        let offset = (page - 1) * per_page;
        let providers = sqlx::query_as::<_, Provider>(
            r#"
            SELECT
                id, owner_id, kind, name, base_url, api_key, extra_headers, is_enabled,
                created_at, updated_at
            FROM providers
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        Ok(providers)
    }
}