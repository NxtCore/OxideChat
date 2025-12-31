-- AI Provider System
-- This migration creates the tables for AI provider management

-- Provider kind enum
CREATE TYPE provider_kind AS ENUM (
    'openai',
    'openai_compat',
    'openrouter',
    'anthropic',
    'google',
    'ollama',
    'lmstudio',
    'custom'
);

-- AI Providers table
-- Stores provider configurations (both system-wide and user-specific BYOK)
CREATE TABLE IF NOT EXISTS ai_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL = system provider
    kind provider_kind NOT NULL,
    name VARCHAR(100) NOT NULL,
    base_url VARCHAR(500) NOT NULL,
    api_key TEXT,  -- Encrypted if ENCRYPTION_KEY set, else plaintext
    extra_headers JSONB DEFAULT '{}',
    is_enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, name)
);

-- AI Models table
-- Stores discovered/configured models from providers
CREATE TABLE IF NOT EXISTS ai_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES ai_providers(id) ON DELETE CASCADE,
    model_id VARCHAR(255) NOT NULL,  -- The model ID used by the provider
    display_name VARCHAR(255) NOT NULL,
    capabilities JSONB DEFAULT '{}',  -- streaming, tools, vision, etc.
    modalities JSONB DEFAULT '["text"]',  -- text, vision, audio, etc.
    context_length INTEGER,
    max_tokens INTEGER,
    is_enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider_id, model_id)
);

-- AI Usage tracking table
-- Tracks API usage for billing, quotas, and analytics
CREATE TABLE IF NOT EXISTS ai_usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    provider_id UUID REFERENCES ai_providers(id) ON DELETE SET NULL,
    model_id UUID REFERENCES ai_models(id) ON DELETE SET NULL,
    request_type VARCHAR(50) NOT NULL,  -- 'chat', 'completion', 'embedding', etc.
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    total_tokens INTEGER DEFAULT 0,
    latency_ms INTEGER,
    success BOOLEAN DEFAULT true,
    error_message TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Configuration for user provider feature
INSERT INTO app_config (key, value) VALUES 
    ('allow_user_providers', 'false')
ON CONFLICT (key) DO NOTHING;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_ai_providers_owner_id ON ai_providers(owner_id);
CREATE INDEX IF NOT EXISTS idx_ai_providers_kind ON ai_providers(kind);
CREATE INDEX IF NOT EXISTS idx_ai_providers_enabled ON ai_providers(is_enabled);
CREATE INDEX IF NOT EXISTS idx_ai_models_provider_id ON ai_models(provider_id);
CREATE INDEX IF NOT EXISTS idx_ai_models_enabled ON ai_models(is_enabled);
CREATE INDEX IF NOT EXISTS idx_ai_usage_user_id ON ai_usage(user_id);
CREATE INDEX IF NOT EXISTS idx_ai_usage_provider_id ON ai_usage(provider_id);
CREATE INDEX IF NOT EXISTS idx_ai_usage_created_at ON ai_usage(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_usage_user_time ON ai_usage(user_id, created_at DESC);
