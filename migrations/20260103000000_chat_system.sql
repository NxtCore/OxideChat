-- Chat System Tables
-- Workspaces, Chats, Messages, and User Preferences for the chat interface

-- Workspaces (linked to users)
CREATE TABLE workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    icon VARCHAR(50),
    color VARCHAR(20),
    sort_order INTEGER DEFAULT 0,
    is_default BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, name)
);

-- Chats (linked to workspaces)
CREATE TABLE chats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID REFERENCES workspaces(id) ON DELETE SET NULL,
    title VARCHAR(255),
    is_pinned BOOLEAN DEFAULT false,
    is_archived BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Messages
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL, -- 'user', 'assistant', 'system'
    content TEXT NOT NULL,
    reasoning_content TEXT, -- For models with extended thinking
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    reasoning_effort VARCHAR(20),
    input_tokens INTEGER,
    output_tokens INTEGER,
    reasoning_tokens INTEGER,
    input_cost_usd DECIMAL(10, 8),     -- Cost for input tokens (nullable)
    output_cost_usd DECIMAL(10, 8),    -- Cost for output tokens (nullable)
    reasoning_cost_usd DECIMAL(10, 8), -- Cost for reasoning tokens (nullable)
    latency_ms INTEGER,                -- Response latency in milliseconds
    reasoning_latency_ms INTEGER,      -- Reasoning latency in milliseconds
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- User preferences (streaming animation, default model, etc.)
CREATE TABLE user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    default_model_key VARCHAR(255),
    favorite_model_keys JSONB DEFAULT '[]',
    streaming_animation VARCHAR(30) DEFAULT 'fade',
    use_remend BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_workspaces_user ON workspaces(user_id);
CREATE INDEX idx_workspaces_default ON workspaces(user_id, is_default) WHERE is_default = true;

CREATE INDEX idx_chats_user ON chats(user_id);
CREATE INDEX idx_chats_workspace ON chats(workspace_id);
CREATE INDEX idx_chats_updated ON chats(updated_at DESC);
CREATE INDEX idx_chats_pinned ON chats(user_id, is_pinned) WHERE is_pinned = true;

CREATE INDEX idx_messages_chat ON messages(chat_id);
CREATE INDEX idx_messages_created ON messages(created_at);