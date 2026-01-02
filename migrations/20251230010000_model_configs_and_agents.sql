-- Model Configuration System
-- This migration adds unified model configs that serve as both capability overrides and custom personas

-- Model configurations (unified: base configs + custom personas/agents)
-- Uses stable_key = "provider_kind:model_id" for persistence across provider reinstalls
-- Also maintains optional direct FK for efficient joins
CREATE TABLE IF NOT EXISTS model_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE, 
    
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    stable_key VARCHAR(255) NOT NULL,
    
    name VARCHAR(100) NOT NULL,
    description TEXT,
    icon VARCHAR(500),
    
    capabilities JSONB,
    input_modalities JSONB,
    output_modalities JSONB,
    context_length INTEGER,
    max_output_tokens INTEGER,
    
    system_prompt TEXT,
    parameters JSONB DEFAULT '{}',
    
    enabled_tools JSONB DEFAULT '[]',
    
    is_public BOOLEAN DEFAULT false,
    is_featured BOOLEAN DEFAULT false,
    is_default BOOLEAN DEFAULT false,
    is_favorite BOOLEAN DEFAULT false,
    is_hidden BOOLEAN DEFAULT false,
    
    category VARCHAR(50),
    tags JSONB DEFAULT '[]',
    usage_count INTEGER DEFAULT 0,
    extra_settings JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, model_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_provider_metadata_kind ON provider_metadata(provider_kind);
CREATE INDEX IF NOT EXISTS idx_provider_metadata_priority ON provider_metadata(priority DESC);
CREATE INDEX IF NOT EXISTS idx_model_configs_owner_id ON model_configs(owner_id);
CREATE INDEX IF NOT EXISTS idx_model_configs_model_id ON model_configs(model_id);
CREATE INDEX IF NOT EXISTS idx_model_configs_stable_key ON model_configs(stable_key);
CREATE INDEX IF NOT EXISTS idx_model_configs_favorite ON model_configs(owner_id, is_favorite) WHERE is_favorite = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_public ON model_configs(is_public) WHERE is_public = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_featured ON model_configs(is_featured) WHERE is_featured = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_category ON model_configs(category);

-- Model/Provider access control
-- Supports both role-based and user-based permissions
CREATE TABLE IF NOT EXISTS model_access (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Target (set one or both for specificity)
    -- NULL provider_id + NULL model_id = global access to everything
    -- provider_id set + NULL model_id = access to all models from that provider
    -- model_id set = access to specific model
    provider_id UUID REFERENCES providers(id) ON DELETE CASCADE,
    model_id UUID REFERENCES models(id) ON DELETE CASCADE,
    
    -- Grantee (set exactly one)
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    
    -- Permissions
    can_use BOOLEAN DEFAULT false,        -- Can make API calls with this model
    can_configure BOOLEAN DEFAULT false,  -- Can create/modify model_configs
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Ensure exactly one grantee type
    CONSTRAINT grantee_check CHECK (
        (role_id IS NOT NULL AND user_id IS NULL) OR
        (role_id IS NULL AND user_id IS NOT NULL)
    )
);

-- Indexes for model_access
CREATE INDEX IF NOT EXISTS idx_model_access_provider ON model_access(provider_id);
CREATE INDEX IF NOT EXISTS idx_model_access_model ON model_access(model_id);
CREATE INDEX IF NOT EXISTS idx_model_access_role ON model_access(role_id);
CREATE INDEX IF NOT EXISTS idx_model_access_user ON model_access(user_id);

-- Grant admin role full access to all models by default
INSERT INTO model_access (role_id, can_use, can_configure)
SELECT id, true, true FROM roles WHERE name = 'admin'
ON CONFLICT DO NOTHING;
