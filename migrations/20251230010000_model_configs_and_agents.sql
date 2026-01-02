-- Model Configuration & Agents System
-- This migration adds model user preferences, provider metadata, and agents

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
