-- Model Configuration & Agents System
-- This migration adds model user preferences, provider metadata, and agents

-- Provider metadata registry (for icons, display names, etc.)
-- This is seeded with known providers and can be updated
CREATE TABLE IF NOT EXISTS provider_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Match by kind first, then by name pattern
    provider_kind provider_kind,
    name_pattern VARCHAR(100),  -- NULL = applies to all of this kind, or regex/glob pattern
    display_name VARCHAR(100) NOT NULL,
    icon_url VARCHAR(500),  -- URL to icon, or data:image/svg+xml;base64,...
    icon_svg TEXT,  -- Inline SVG content (preferred for built-in providers)
    brand_color VARCHAR(20),  -- Hex color like #FF5500
    website_url VARCHAR(500),
    documentation_url VARCHAR(500),
    is_builtin BOOLEAN DEFAULT false,  -- Built-in entries can't be deleted
    priority INTEGER DEFAULT 0,  -- Higher = preferred when multiple match
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Model user configurations (persists across provider reinstalls)
-- Uses stable_key = "provider_kind:model_id" for persistence
CREATE TABLE IF NOT EXISTS model_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL = system default
    stable_key VARCHAR(255) NOT NULL,  -- e.g., "openai:gpt-4o", "anthropic:claude-3-5-sonnet"
    
    -- Display customization
    display_name VARCHAR(100),  -- Custom display name
    description TEXT,
    icon_override VARCHAR(500),  -- Override the default icon
    
    -- Default parameters
    default_temperature REAL,
    default_max_tokens INTEGER,
    default_top_p REAL,
    default_frequency_penalty REAL,
    default_presence_penalty REAL,
    
    -- Capabilities/restrictions
    is_favorite BOOLEAN DEFAULT false,
    is_hidden BOOLEAN DEFAULT false,  -- Hide from model picker
    
    -- Extra settings as JSON
    extra_settings JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, stable_key)
);

-- Agents (reusable AI personas/configurations)
CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL = system agent
    
    -- Identity
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(100),  -- URL-friendly identifier
    description TEXT,
    icon_url VARCHAR(500),
    icon_emoji VARCHAR(10),  -- Alternative: use an emoji
    
    -- Model configuration
    model_stable_key VARCHAR(255),  -- Preferred model (can be overridden at runtime)
    fallback_model_keys JSONB DEFAULT '[]',  -- Fallback models if primary unavailable
    
    -- Behavior
    system_prompt TEXT,
    initial_messages JSONB DEFAULT '[]',  -- Pre-filled conversation starters
    
    -- Parameters
    temperature REAL,
    max_tokens INTEGER,
    top_p REAL,
    frequency_penalty REAL,
    presence_penalty REAL,
    
    -- Tools & capabilities
    enabled_tools JSONB DEFAULT '[]',  -- List of tool names this agent can use
    web_search_enabled BOOLEAN DEFAULT false,
    code_execution_enabled BOOLEAN DEFAULT false,
    
    -- Visibility
    is_public BOOLEAN DEFAULT false,  -- Can other users use this agent?
    is_featured BOOLEAN DEFAULT false,  -- Show in featured list
    is_default BOOLEAN DEFAULT false,  -- Default agent for new chats
    
    -- Metadata
    category VARCHAR(50),  -- e.g., "coding", "writing", "general"
    tags JSONB DEFAULT '[]',
    usage_count INTEGER DEFAULT 0,  -- Track popularity
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, slug)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_provider_metadata_kind ON provider_metadata(provider_kind);
CREATE INDEX IF NOT EXISTS idx_provider_metadata_priority ON provider_metadata(priority DESC);
CREATE INDEX IF NOT EXISTS idx_model_configs_user_id ON model_configs(user_id);
CREATE INDEX IF NOT EXISTS idx_model_configs_stable_key ON model_configs(stable_key);
CREATE INDEX IF NOT EXISTS idx_model_configs_favorite ON model_configs(user_id, is_favorite) WHERE is_favorite = true;
CREATE INDEX IF NOT EXISTS idx_agents_owner_id ON agents(owner_id);
CREATE INDEX IF NOT EXISTS idx_agents_public ON agents(is_public) WHERE is_public = true;
CREATE INDEX IF NOT EXISTS idx_agents_featured ON agents(is_featured) WHERE is_featured = true;
CREATE INDEX IF NOT EXISTS idx_agents_category ON agents(category);

-- Seed built-in provider metadata
INSERT INTO provider_metadata (provider_kind, display_name, icon_svg, brand_color, website_url, is_builtin, priority) VALUES
    -- Native providers
    ('openai', 'OpenAI', '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.896zm16.597 3.855l-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023l-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135l-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08-4.778 2.758a.795.795 0 0 0-.393.681zm1.097-2.365l2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z"/></svg>', '#10A37F', 'https://openai.com', true, 100),
    ('anthropic', 'Anthropic', '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M17.604 3.332L12.002 18.662l-2.136-5.767-5.968 2.168 5.166-8.396H4.372l7.63-3.335zM12 0C5.373 0 0 5.373 0 12s5.373 12 12 12 12-5.373 12-12S18.627 0 12 0z"/></svg>', '#D97757', 'https://anthropic.com', true, 100),
    ('google', 'Google AI', '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12.48 10.92v3.28h7.84c-.24 1.84-.853 3.187-1.787 4.133-1.147 1.147-2.933 2.4-6.053 2.4-4.827 0-8.6-3.893-8.6-8.72s3.773-8.72 8.6-8.72c2.6 0 4.507 1.027 5.907 2.347l2.307-2.307C18.747 1.44 16.133 0 12.48 0 5.867 0 .307 5.387.307 12s5.56 12 12.173 12c3.573 0 6.267-1.173 8.373-3.36 2.16-2.16 2.84-5.213 2.84-7.667 0-.76-.053-1.467-.173-2.053H12.48z"/></svg>', '#4285F4', 'https://ai.google', true, 100),
    ('ollama', 'Ollama', '<svg viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="10"/></svg>', '#FFFFFF', 'https://ollama.ai', true, 100),
    ('lmstudio', 'LM Studio', '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>', '#1E1E1E', 'https://lmstudio.ai', true, 100),
    ('openrouter', 'OpenRouter', '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>', '#6366F1', 'https://openrouter.ai', true, 50)
ON CONFLICT DO NOTHING;

-- Add name-specific overrides for OpenAI Compatible providers
INSERT INTO provider_metadata (provider_kind, name_pattern, display_name, brand_color, website_url, is_builtin, priority) VALUES
    ('openai_compat', 'groq', 'Groq', '#F55036', 'https://groq.com', true, 60),
    ('openai_compat', 'together', 'Together AI', '#000000', 'https://together.ai', true, 60),
    ('openai_compat', 'fireworks', 'Fireworks AI', '#FF6B35', 'https://fireworks.ai', true, 60),
    ('openai_compat', 'deepseek', 'DeepSeek', '#0066FF', 'https://deepseek.com', true, 60),
    ('openai_compat', 'mistral', 'Mistral AI', '#F7D046', 'https://mistral.ai', true, 60),
    ('openai_compat', 'perplexity', 'Perplexity', '#20808D', 'https://perplexity.ai', true, 60),
    ('openai_compat', 'moonshot', 'Moonshot AI', '#1A1A2E', 'https://moonshot.cn', true, 60),
    ('openai_compat', 'x.ai', 'xAI', '#000000', 'https://x.ai', true, 60),
    ('openai_compat', 'grok', 'xAI Grok', '#000000', 'https://x.ai', true, 60)
ON CONFLICT DO NOTHING;
