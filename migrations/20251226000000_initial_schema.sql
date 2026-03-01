-- Initial schema for OxideChat
-- This migration creates the core tables for the chat application

-- Translations table - stores all i18n translations
CREATE TABLE IF NOT EXISTS i18n_translations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    language VARCHAR(10) NOT NULL,
    key_path VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    is_override BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(language, key_path)
);

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) NOT NULL UNIQUE,
    username VARCHAR(100) NOT NULL UNIQUE,
    password_hash VARCHAR(255),  -- NULL for OAuth/LDAP users
    auth_method VARCHAR(50) NOT NULL DEFAULT 'local',  -- 'local', 'oauth', 'ldap'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Roles table
CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(50) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- User-Roles junction table (n:m)
CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);

-- External identity providers (OAuth, LDAP, etc.)
CREATE TABLE IF NOT EXISTS user_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL,           -- 'google', 'github', 'ldap', etc.
    provider_user_id VARCHAR(255) NOT NULL,  -- External ID from provider
    provider_data JSONB,                     -- Tokens, profile info, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_user_id)
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Stores key-value pairs for global application settings

CREATE TABLE IF NOT EXISTS app_config (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default configuration values
INSERT INTO app_config (key, value) VALUES
    ('language', 'en'),
    ('allow_user_providers', 'false'),
    ('default_theme', '{}'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Rate limits table for customizable endpoint rate limiting
-- TODO: Implement tower-governor middleware that reads from this table
CREATE TABLE IF NOT EXISTS rate_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_pattern VARCHAR(255) NOT NULL UNIQUE,  -- e.g., "/api/v1/auth/login", "/api/v1/*"
    requests_per_window INT NOT NULL,
    window_seconds INT NOT NULL,
    scope VARCHAR(50) NOT NULL DEFAULT 'ip',  -- 'ip', 'user', 'global'
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Default rate limits for auth endpoints
INSERT INTO rate_limits (endpoint_pattern, requests_per_window, window_seconds, scope) VALUES
    ('/api/v1/auth/login', 5, 900, 'ip'),      -- 5 requests per 15 minutes
    ('/api/v1/auth/register', 3, 3600, 'ip'),  -- 3 requests per hour
    ('/api/v1/auth/setup', 3, 3600, 'ip')      -- 3 requests per hour
ON CONFLICT (endpoint_pattern) DO NOTHING;

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event VARCHAR(50) NOT NULL,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    target_type VARCHAR(50),
    target_id UUID,
    resource_type VARCHAR(50),
    resource_id UUID,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);
DO $$ BEGIN
    CREATE TYPE provider_kind AS ENUM (
        'OPENAI',
        'OPENAI_COMPAT',
        'OPENROUTER',
        'ANTHROPIC',
        'GOOGLE',
        'CUSTOM'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS providers (
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

CREATE TABLE IF NOT EXISTS models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id VARCHAR(255) NOT NULL,  -- The model ID used by the provider
    display_name VARCHAR(255) NOT NULL,
    capabilities JSONB DEFAULT '{}',  -- streaming, tools, vision, etc.
    input_modalities JSONB DEFAULT '["text"]',  
    output_modalities JSONB DEFAULT '["text"]', 
    context_length INTEGER,
    max_tokens INTEGER,
    is_enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider_id, model_id)
);

CREATE TABLE IF NOT EXISTS usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    provider_id UUID REFERENCES providers(id) ON DELETE SET NULL,
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
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
    sampling JSONB DEFAULT '{}',
    
    enabled_tools JSONB DEFAULT '[]',
    
    is_public BOOLEAN DEFAULT false,
    is_featured BOOLEAN DEFAULT false,
    is_default BOOLEAN DEFAULT false,
    is_favorite BOOLEAN DEFAULT false,

    category VARCHAR(50),
    tags JSONB DEFAULT '[]',
    usage_count INTEGER DEFAULT 0,
    extra_settings JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, model_id)
);

CREATE TABLE IF NOT EXISTS model_access (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    provider_id UUID REFERENCES providers(id) ON DELETE CASCADE,
    model_id UUID REFERENCES models(id) ON DELETE CASCADE,
    
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    
    can_use BOOLEAN DEFAULT false,
    can_configure BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT grantee_check CHECK (
        (role_id IS NOT NULL AND user_id IS NULL) OR
        (role_id IS NULL AND user_id IS NOT NULL)
    )
);

-- Workspaces (linked to users)
CREATE TABLE IF NOT EXISTS workspaces (
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
CREATE TABLE IF NOT EXISTS chats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID REFERENCES workspaces(id) ON DELETE SET NULL,
    title VARCHAR(255),
    is_pinned BOOLEAN DEFAULT false,
    is_archived BOOLEAN DEFAULT false,
    branched_from_chat_id UUID REFERENCES chats(id) ON DELETE SET NULL,
    branched_from_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL,
    content TEXT NOT NULL,
    reasoning_content TEXT, 
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    parent_id UUID REFERENCES messages(id) ON DELETE CASCADE,
    fork_index INTEGER NOT NULL DEFAULT 1,
    is_active_fork BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    content_parts JSONB DEFAULT '[]',
    cost_details JSONB DEFAULT '{}',
    usage_details JSONB DEFAULT '{}',
    reasoning_details JSONB DEFAULT '{}'
);

-- User preferences (streaming animation, default model, etc.)
CREATE TABLE IF NOT EXISTS user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    default_model_key VARCHAR(255),
    favorite_model_keys JSONB DEFAULT '[]',
    streaming_animation VARCHAR(30) DEFAULT 'fade',
    use_remend BOOLEAN DEFAULT true,
    theme_css_vars JSONB DEFAULT '{}',
    custom_theme_urls JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    data BYTEA,  -- NULL if using file storage
    file_path VARCHAR(500),  -- Path relative to storage root (for file storage)
    mime_type VARCHAR(64) NOT NULL DEFAULT 'image/png',
	size_bytes BIGINT NOT NULL,
    source VARCHAR(50),  -- 'imagegen', 'upload', etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO roles (name) VALUES ('admin'), ('user') ON CONFLICT DO NOTHING;

INSERT INTO permissions (name, description) VALUES
    ('settings.*', 'All settings permissions (wildcard)'),
    ('admin.*', 'All admin permissions (wildcard)'),
    ('settings.profile.view', 'View own profile'),
    ('settings.profile.edit', 'Edit own profile'),
    ('settings.sessions.view', 'View active sessions'),
    ('settings.sessions.revoke', 'Revoke sessions'),
    ('settings.appearance.edit', 'Change appearance settings'),
    ('admin.users.view', 'View all users'),
    ('admin.users.edit', 'Edit user accounts'),
    ('admin.config.view', 'View application config'),
    ('admin.config.edit', 'Edit application config'),
    ('admin.providers.view', 'View AI provider configuration'),
    ('admin.providers.edit', 'Configure AI providers')
ON CONFLICT (name) DO NOTHING;

-- Assign permissions to user role (basic settings)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'user' AND p.name LIKE 'settings.%' AND p.name != 'settings.*'
ON CONFLICT DO NOTHING;

-- Assign wildcard permissions to admin role (covers all current and future permissions)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin' AND p.name IN ('admin.*', 'settings.*')
ON CONFLICT DO NOTHING;

-- Grant admin role full access to all models by default
INSERT INTO model_access (role_id, can_use, can_configure)
SELECT id, true, true FROM roles WHERE name = 'admin'
ON CONFLICT DO NOTHING;

-- ============= Tool Calling Infrastructure =============

-- Tool source types
DO $$ BEGIN
    CREATE TYPE tool_source_kind AS ENUM (
        'BUILTIN',       -- Built-in tools (Exa search, etc.)
        'WASM',          -- Extism WASM plugins  
        'MCP',           -- MCP server connection
        'HTTP'           -- HTTP endpoint tools
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- WASM blobs storage for compiled plugins
CREATE TABLE IF NOT EXISTS wasm_blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,
    original_filename VARCHAR(255),
    compiled_from VARCHAR(50),  -- 'rust', 'javascript', 'wasm' (direct upload)
    blob BYTEA NOT NULL,
    size_bytes INTEGER NOT NULL,
    sha256_hash VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tools table - defines available tools
CREATE TABLE IF NOT EXISTS tools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL = system tool
    
    name VARCHAR(100) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    icon VARCHAR(500),
    
    source_kind tool_source_kind NOT NULL,
    source_config JSONB NOT NULL DEFAULT '{}',  -- Kind-specific config
    
    -- JSON Schema for tool parameters (passed to LLM)
    input_schema JSONB NOT NULL,
    
    -- Custom settings schema (API keys, user config) - NOT passed to LLM
    settings_schema JSONB DEFAULT '{}',
    
    -- Permissions
    is_enabled BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, name)
);

-- User-provided settings for tools (API keys, etc.)
CREATE TABLE IF NOT EXISTS user_tool_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    tool_id UUID NOT NULL REFERENCES tools(id) ON DELETE CASCADE,
    settings JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partial unique index for system-wide settings (user_id IS NULL)
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tool_settings_tool_null_user ON user_tool_settings(tool_id)
WHERE user_id IS NULL;

-- Partial unique index for per-user settings (user_id IS NOT NULL)
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tool_settings_user_tool_not_null ON user_tool_settings(user_id, tool_id)
WHERE user_id IS NOT NULL;

-- MCP Servers - for MCP tool sources (supports stdio and SSE transports)
CREATE TABLE IF NOT EXISTS mcp_servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,
    
    name VARCHAR(100) NOT NULL,
    transport VARCHAR(50) NOT NULL,  -- 'stdio' or 'sse'
    connection_config JSONB NOT NULL,  -- {command, args} for stdio, {url, headers} for SSE
    
    is_enabled BOOLEAN DEFAULT true,
    last_health_check TIMESTAMPTZ,
    health_status VARCHAR(50),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, name)
);

-- Create tool_functions table
CREATE TABLE IF NOT EXISTS tool_functions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_id UUID NOT NULL REFERENCES tools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    input_schema JSONB NOT NULL,
    entrypoint VARCHAR(255),               -- Optional override for WASM/HTTP routing
    sort_order INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tool_id, name)
);

-- Tool executions - audit log for tool calls
CREATE TABLE IF NOT EXISTS tool_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    tool_id UUID REFERENCES tools(id) ON DELETE SET NULL,
    tool_function UUID REFERENCES tool_functions(id) ON DELETE SET NULL,
    
    tool_call_id VARCHAR(255) NOT NULL,  -- From LLM
    input_args JSONB NOT NULL,
    output JSONB,
    error TEXT,
    
    execution_ms INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_i18n_translations_language ON i18n_translations(language);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_auth_method ON users(auth_method);
CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles(role_id);
CREATE INDEX IF NOT EXISTS idx_user_identities_user_id ON user_identities(user_id);
CREATE INDEX IF NOT EXISTS idx_user_identities_provider ON user_identities(provider, provider_user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_app_config_key ON app_config(key);
CREATE INDEX IF NOT EXISTS idx_audit_logs_event ON audit_logs(event);
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor_id ON audit_logs(actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_target ON audit_logs(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_event_time ON audit_logs(event, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_role_permissions_role_id ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id ON role_permissions(permission_id);
CREATE INDEX IF NOT EXISTS idx_providers_owner_id ON providers(owner_id);
CREATE INDEX IF NOT EXISTS idx_providers_kind ON providers(kind);
CREATE INDEX IF NOT EXISTS idx_providers_enabled ON providers(is_enabled);
CREATE INDEX IF NOT EXISTS idx_models_provider_id ON models(provider_id);
CREATE INDEX IF NOT EXISTS idx_models_enabled ON models(is_enabled);
CREATE INDEX IF NOT EXISTS idx_usage_user_id ON usage(user_id);
CREATE INDEX IF NOT EXISTS idx_usage_provider_id ON usage(provider_id);
CREATE INDEX IF NOT EXISTS idx_usage_created_at ON usage(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_usage_user_time ON usage(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_model_configs_owner_id ON model_configs(owner_id);
CREATE INDEX IF NOT EXISTS idx_model_configs_model_id ON model_configs(model_id);
CREATE INDEX IF NOT EXISTS idx_model_configs_stable_key ON model_configs(stable_key);
CREATE INDEX IF NOT EXISTS idx_model_configs_favorite ON model_configs(owner_id, is_favorite) WHERE is_favorite = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_public ON model_configs(is_public) WHERE is_public = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_featured ON model_configs(is_featured) WHERE is_featured = true;
CREATE INDEX IF NOT EXISTS idx_model_configs_category ON model_configs(category);
CREATE UNIQUE INDEX IF NOT EXISTS model_configs_system_unique ON model_configs (model_id) WHERE owner_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_model_access_provider ON model_access(provider_id);
CREATE INDEX IF NOT EXISTS idx_model_access_model ON model_access(model_id);
CREATE INDEX IF NOT EXISTS idx_model_access_role ON model_access(role_id);
CREATE INDEX IF NOT EXISTS idx_model_access_user ON model_access(user_id);
CREATE INDEX IF NOT EXISTS idx_workspaces_user ON workspaces(user_id);
CREATE INDEX IF NOT EXISTS idx_workspaces_default ON workspaces(user_id, is_default) WHERE is_default = true;
CREATE INDEX IF NOT EXISTS idx_chats_user ON chats(user_id);
CREATE INDEX IF NOT EXISTS idx_chats_workspace ON chats(workspace_id);
CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chats_pinned ON chats(user_id, is_pinned) WHERE is_pinned = true;
CREATE INDEX IF NOT EXISTS idx_chats_branched_from ON chats(branched_from_chat_id) WHERE branched_from_chat_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_messages_chat ON messages(chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at);
CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_id);
CREATE INDEX IF NOT EXISTS idx_messages_fork ON messages(parent_id, fork_index);
CREATE INDEX IF NOT EXISTS idx_images_created_at ON images(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_images_user_id ON images(user_id);
CREATE INDEX IF NOT EXISTS idx_messages_content_parts_gin ON messages USING GIN (content_parts);

-- Tool indexes
CREATE INDEX IF NOT EXISTS idx_wasm_blobs_owner ON wasm_blobs(owner_id);
CREATE INDEX IF NOT EXISTS idx_wasm_blobs_hash ON wasm_blobs(sha256_hash);
CREATE INDEX IF NOT EXISTS idx_tools_owner ON tools(owner_id);
CREATE INDEX IF NOT EXISTS idx_tools_source ON tools(source_kind);
CREATE INDEX IF NOT EXISTS idx_tools_enabled ON tools(is_enabled) WHERE is_enabled = true;
CREATE INDEX IF NOT EXISTS idx_user_tool_settings_user ON user_tool_settings(user_id);
CREATE INDEX IF NOT EXISTS idx_user_tool_settings_tool ON user_tool_settings(tool_id);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_owner ON mcp_servers(owner_id);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_enabled ON mcp_servers(is_enabled) WHERE is_enabled = true;
CREATE INDEX IF NOT EXISTS idx_tool_executions_message ON tool_executions(message_id);
CREATE INDEX IF NOT EXISTS idx_tool_executions_tool ON tool_executions(tool_id);
CREATE INDEX IF NOT EXISTS idx_tool_executions_created ON tool_executions(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tool_executions_function ON tool_executions(tool_function);
CREATE INDEX IF NOT EXISTS idx_tool_functions_tool ON tool_functions(tool_id);
CREATE INDEX IF NOT EXISTS idx_tool_functions_name ON tool_functions(tool_id, name);


--- Translations
INSERT INTO i18n_translations (language, key_path, value) VALUES
    -- Common
    ('en', 'common.copy_to_clipboard', 'Copied to clipboard'),
    ('de', 'common.copy_to_clipboard', 'In die Zwischenablage kopiert'),
    ('en', 'common.create', 'Create'),
    ('de', 'common.create', 'Erstellen'),
    ('en', 'common.next', 'Next'),
    ('de', 'common.next', 'Nächste'),
    ('en', 'common.previous', 'Previous'),
    ('de', 'common.previous', 'Vorherige'),
    ('en', 'common.error', 'Error'),
    ('en', 'common.loading', 'Loading...'),
    ('en', 'common.user', 'User'),
    ('de', 'common.error', 'Fehler'),
    ('de', 'common.loading', 'Wird geladen...'),
    ('de', 'common.user', 'Benutzer'),
    ('en', 'common.cancel', 'Cancel'),
    ('en', 'common.save', 'Save'),
    ('en', 'common.delete', 'Delete'),
    ('de', 'common.cancel', 'Abbrechen'),
    ('de', 'common.save', 'Speichern'),
    ('de', 'common.delete', 'Löschen'),
    ('en', 'common.close', 'Close'),
    ('de', 'common.close', 'Schließen'),

    -- Auth errors
    ('en', 'auth.errors.setup_completed', 'Setup has already been completed'),
    ('en', 'auth.errors.setup_required', 'Setup must be completed first'),
    ('en', 'auth.errors.invalid_email', 'Please enter a valid email address'),
    ('en', 'auth.errors.username_too_short', 'Username must be at least 3 characters'),
    ('en', 'auth.errors.username_too_long', 'Username must be at most 32 characters'),
    ('en', 'auth.errors.username_invalid_chars', 'Username can only contain letters, numbers, underscores, and hyphens'),
    ('en', 'auth.errors.password_too_short', 'Password must be at least 8 characters'),
    ('en', 'auth.errors.email_or_username_taken', 'Email or username already taken'),
    ('en', 'auth.errors.invalid_credentials', 'Invalid email or password'),
    ('en', 'auth.errors.external_auth', 'This account uses external authentication'),
    ('en', 'auth.errors.not_authenticated', 'Not authenticated'),
    ('en', 'auth.errors.internal_error', 'An error occurred. Please try again.'),
    ('en', 'auth.messages.logout_success', 'Logged out successfully'),
    ('en', 'auth.errors.oauth_email_conflict', 'An account with this email already exists. Please log in with your original method.'),
    ('en', 'auth.errors.oauth_email_conflict_title', 'Account Already Exists'),
    ('en', 'auth.errors.oauth_email_conflict_help', 'Your OAuth provider returned an email that is already associated with an existing account. To link your OAuth identity, please log in with your existing credentials first, then link the OAuth provider from your account settings.'),
    ('en', 'auth.errors.oauth_state_mismatch', 'OAuth authentication failed due to a state mismatch. Please try again.'),
    ('en', 'auth.errors.oauth_state_mismatch_title', 'Authentication Failed'),
    ('en', 'auth.errors.oauth_token_error', 'Failed to obtain authentication token. Please try again.'),
    ('en', 'auth.errors.oauth_token_error_title', 'Token Error'),
    ('en', 'auth.errors.oauth_user_info_error', 'Failed to retrieve user information from the OAuth provider.'),
    ('en', 'auth.errors.oauth_user_info_error_title', 'User Info Error'),
    ('en', 'auth.errors.unknown_error', 'An unexpected error occurred. Please try again.'),
    ('en', 'auth.errors.unknown_error_title', 'Error'),
    ('en', 'auth.errors.back_to_login', 'Back to Login'),
    ('en', 'auth.errors.go_home', 'Go Home'),
    ('en', 'auth.errors.email_taken', 'This email is already registered'),
    ('en', 'auth.errors.username_taken', 'This username is already taken'),

    ('de', 'auth.errors.setup_completed', 'Die Einrichtung wurde bereits abgeschlossen'),
    ('de', 'auth.errors.setup_required', 'Die Einrichtung muss zuerst abgeschlossen werden'),
    ('de', 'auth.errors.invalid_email', 'Bitte geben Sie eine gültige E-Mail-Adresse ein'),
    ('de', 'auth.errors.username_too_short', 'Der Benutzername muss mindestens 3 Zeichen haben'),
    ('de', 'auth.errors.username_too_long', 'Der Benutzername darf maximal 32 Zeichen haben'),
    ('de', 'auth.errors.username_invalid_chars', 'Der Benutzername darf nur Buchstaben, Zahlen, Unterstriche und Bindestriche enthalten'),
    ('de', 'auth.errors.password_too_short', 'Das Passwort muss mindestens 8 Zeichen haben'),
    ('de', 'auth.errors.email_or_username_taken', 'E-Mail oder Benutzername bereits vergeben'),
    ('de', 'auth.errors.invalid_credentials', 'Ungültige E-Mail oder Passwort'),
    ('de', 'auth.errors.external_auth', 'Dieses Konto verwendet externe Authentifizierung'),
    ('de', 'auth.errors.not_authenticated', 'Nicht authentifiziert'),
    ('de', 'auth.errors.internal_error', 'Ein Fehler ist aufgetreten. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.messages.logout_success', 'Erfolgreich abgemeldet'),
    ('de', 'auth.errors.oauth_email_conflict', 'Ein Konto mit dieser E-Mail-Adresse existiert bereits. Bitte melden Sie sich mit Ihrer ursprünglichen Methode an.'),
    ('de', 'auth.errors.oauth_email_conflict_title', 'Konto existiert bereits'),
    ('de', 'auth.errors.oauth_email_conflict_help', 'Ihr OAuth-Anbieter hat eine E-Mail-Adresse zurückgegeben, die bereits mit einem bestehenden Konto verknüpft ist. Um Ihre OAuth-Identität zu verknüpfen, melden Sie sich bitte zuerst mit Ihren bestehenden Anmeldedaten an und verknüpfen Sie dann den OAuth-Anbieter in Ihren Kontoeinstellungen.'),
    ('de', 'auth.errors.oauth_state_mismatch', 'OAuth-Authentifizierung aufgrund einer Statusabweichung fehlgeschlagen. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.oauth_state_mismatch_title', 'Authentifizierung fehlgeschlagen'),
    ('de', 'auth.errors.oauth_token_error', 'Authentifizierungstoken konnte nicht abgerufen werden. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.oauth_token_error_title', 'Token-Fehler'),
    ('de', 'auth.errors.oauth_user_info_error', 'Benutzerinformationen konnten nicht vom OAuth-Anbieter abgerufen werden.'),
    ('de', 'auth.errors.oauth_user_info_error_title', 'Benutzerinfo-Fehler'),
    ('de', 'auth.errors.unknown_error', 'Ein unerwarteter Fehler ist aufgetreten. Bitte versuchen Sie es erneut.'),
    ('de', 'auth.errors.unknown_error_title', 'Fehler'),
    ('de', 'auth.errors.back_to_login', 'Zurück zur Anmeldung'),
    ('de', 'auth.errors.go_home', 'Zur Startseite'),
    ('de', 'auth.errors.email_taken', 'Diese E-Mail-Adresse ist bereits registriert'),
    ('de', 'auth.errors.username_taken', 'Dieser Benutzername ist bereits vergeben'),

    -- Sidebar
    ('en', 'sidebar.chats', 'Chats'),
    ('en', 'sidebar.new_chat', 'New Chat'),
    ('en', 'sidebar.logout', 'Log out'),
    ('en', 'sidebar.ai_chat_app', 'AI Chat Application'),
    ('en', 'sidebar.workspace', 'Workspace'),
    ('en', 'sidebar.all_chats', 'All chats'),
    ('en', 'sidebar.all', 'All'),
    ('de', 'sidebar.chats', 'Chats'),
    ('de', 'sidebar.new_chat', 'Neuer Chat'),
    ('de', 'sidebar.logout', 'Abmelden'),
    ('de', 'sidebar.ai_chat_app', 'KI-Chat-Anwendung'),
    ('de', 'sidebar.workspace', 'Arbeitsbereich'),
    ('de', 'sidebar.all_chats', 'Alle Chats'),
    ('de', 'sidebar.all', 'Alle'),

    -- Auth: Login
    ('en', 'auth.login.title', 'Welcome back'),
    ('en', 'auth.login.description', 'Login with your favorite external provider'),
    ('en', 'auth.login.google', 'Login with Google'),
    ('en', 'auth.login.discord', 'Login with Discord'),
    ('en', 'auth.login.or_continue', 'Or continue with'),
    ('en', 'auth.login.email', 'Email'),
    ('en', 'auth.login.password', 'Password'),
    ('en', 'auth.login.forgot_password', 'Forgot your password?'),
    ('en', 'auth.login.submit', 'Login'),
    ('en', 'auth.login.submitting', 'Signing in...'),
    ('en', 'auth.login.no_account', 'Don''t have an account?'),
    ('en', 'auth.login.sign_up', 'Sign up'),
    ('en', 'auth.login.terms_prefix', 'By clicking continue, you agree to our'),
    ('en', 'auth.login.terms_link', 'Terms of Service'),
    ('en', 'auth.login.and', 'and'),
    ('en', 'auth.login.privacy_link', 'Privacy Policy'),

    ('de', 'auth.login.title', 'Willkommen zurück'),
    ('de', 'auth.login.description', 'Melden Sie sich mit einem externen Provider an'),
    ('de', 'auth.login.google', 'Mit Google anmelden'),
    ('de', 'auth.login.discord', 'Mit Discord anmelden'),
    ('de', 'auth.login.or_continue', 'Oder weiter mit'),
    ('de', 'auth.login.email', 'E-Mail'),
    ('de', 'auth.login.password', 'Passwort'),
    ('de', 'auth.login.forgot_password', 'Passwort vergessen?'),
    ('de', 'auth.login.submit', 'Anmelden'),
    ('de', 'auth.login.submitting', 'Anmeldung...'),
    ('de', 'auth.login.no_account', 'Noch kein Konto?'),
    ('de', 'auth.login.sign_up', 'Registrieren'),
    ('de', 'auth.login.terms_prefix', 'Durch Klicken auf "Weiter" erklären Sie sich mit unseren'),
    ('de', 'auth.login.terms_link', 'Nutzungsbedingungen'),
    ('de', 'auth.login.and', 'und'),
    ('de', 'auth.login.privacy_link', 'Datenschutzbestimmungen'),

    -- Auth: Register
    ('en', 'auth.register.title', 'Create an account'),
    ('en', 'auth.register.description', 'Sign up with your favorite external provider'),
    ('en', 'auth.register.google', 'Sign up with Google'),
    ('en', 'auth.register.discord', 'Sign up with Discord'),
    ('en', 'auth.register.username', 'Username'),
    ('en', 'auth.register.confirm_password', 'Confirm Password'),
    ('en', 'auth.register.submit', 'Create Account'),
    ('en', 'auth.register.submitting', 'Creating account...'),
    ('en', 'auth.register.have_account', 'Already have an account?'),
    ('en', 'auth.register.sign_in', 'Sign in'),
    ('en', 'auth.register.passwords_mismatch', 'Passwords do not match'),
    ('en', 'auth.register.password_requirements', 'At least 8 characters'),

    ('de', 'auth.register.title', 'Konto erstellen'),
    ('de', 'auth.register.description', 'Registrieren Sie sich mit einem externen Provider'),
    ('de', 'auth.register.google', 'Mit Google registrieren'),
    ('de', 'auth.register.discord', 'Mit Discord registrieren'),
    ('de', 'auth.register.username', 'Benutzername'),
    ('de', 'auth.register.confirm_password', 'Passwort bestätigen'),
    ('de', 'auth.register.submit', 'Konto erstellen'),
    ('de', 'auth.register.submitting', 'Konto wird erstellt...'),
    ('de', 'auth.register.have_account', 'Haben Sie bereits ein Konto?'),
    ('de', 'auth.register.sign_in', 'Anmelden'),
    ('de', 'auth.register.passwords_mismatch', 'Passwörter stimmen nicht überein'),
    ('de', 'auth.register.password_requirements', 'Mindestens 8 Zeichen'),

    -- Auth: Setup
    ('en', 'auth.setup.title', 'Welcome to OxideChat'),
    ('en', 'auth.setup.description', 'No users exist yet. Create your admin account to get started.'),
    ('en', 'auth.setup.submit', 'Create Admin Account'),
    ('en', 'auth.setup.one_time_tip', 'This is a one-time setup. The first account will have administrator privileges.'),

    ('de', 'auth.setup.title', 'Willkommen bei OxideChat'),
    ('de', 'auth.setup.description', 'Es existieren noch keine Benutzer. Erstellen Sie Ihr Admin-Konto, um zu beginnen.'),
    ('de', 'auth.setup.submit', 'Admin-Konto erstellen'),
    ('de', 'auth.setup.one_time_tip', 'Dies ist eine einmalige Einrichtung. Das erste Konto erhält Administratorrechte.'),

    -- Welcome Page
    ('en', 'welcome.title', 'Welcome to OxideChat'),
    ('en', 'welcome.description', 'Select a chat from the sidebar or start a new conversation'),
    ('de', 'welcome.title', 'Willkommen bei OxideChat'),
    ('de', 'welcome.description', 'Wählen Sie einen Chat aus der Seitenleiste oder beginnen Sie eine neue Konversation'),

    -- Calendar Component (used in DialogCalendar.vue)
    ('en', 'guild.components.dialog_calendar.date.select', 'Select Date'),
    ('en', 'guild.components.dialog_calendar.time.label', 'Time'),
    ('en', 'guild.components.dialog_calendar.time.select', 'Select Time'),
    ('en', 'guild.components.dialog_calendar.reset', 'Reset'),
    ('en', 'guild.components.dialog_calendar.ok', 'OK'),

    ('de', 'guild.components.dialog_calendar.date.select', 'Datum auswählen'),
    ('de', 'guild.components.dialog_calendar.time.label', 'Uhrzeit'),
    ('de', 'guild.components.dialog_calendar.time.select', 'Uhrzeit auswählen'),
    ('de', 'guild.components.dialog_calendar.reset', 'Zurücksetzen'),
    ('de', 'guild.components.dialog_calendar.ok', 'OK'),
 
    ('en', 'auth.errors.password_too_long', 'Password must be at most 128 characters'),
    ('en', 'auth.errors.password_no_uppercase', 'Password must contain at least one uppercase letter'),
    ('en', 'auth.errors.password_no_lowercase', 'Password must contain at least one lowercase letter'),
    ('en', 'auth.errors.password_no_digit', 'Password must contain at least one number'),
    ('en', 'auth.errors.password_no_special', 'Password must contain at least one special character'),
    ('en', 'auth.errors.password_requirements_not_met', 'Please ensure your password meets all requirements'),
    
    -- Password validation error messages (German)
    ('de', 'auth.errors.password_too_long', 'Das Passwort darf maximal 128 Zeichen haben'),
    ('de', 'auth.errors.password_no_uppercase', 'Das Passwort muss mindestens einen Großbuchstaben enthalten'),
    ('de', 'auth.errors.password_no_lowercase', 'Das Passwort muss mindestens einen Kleinbuchstaben enthalten'),
    ('de', 'auth.errors.password_no_digit', 'Das Passwort muss mindestens eine Zahl enthalten'),
    ('de', 'auth.errors.password_no_special', 'Das Passwort muss mindestens ein Sonderzeichen enthalten'),
    ('de', 'auth.errors.password_requirements_not_met', 'Bitte stellen Sie sicher, dass Ihr Passwort alle Anforderungen erfüllt'),
    
    ('en', 'error_messages.theme_import_failed', 'Failed to import theme'),
    ('de', 'error_messages.theme_import_failed', 'Fehler beim Importieren des Themes'),
    
    -- Password strength indicator labels (English)
    ('en', 'auth.password.min_length', 'At least {min} characters'),
    ('en', 'auth.password.max_length', 'Maximum {max} characters'),
    ('en', 'auth.password.uppercase', 'One uppercase letter'),
    ('en', 'auth.password.lowercase', 'One lowercase letter'),
    ('en', 'auth.password.digit', 'One number'),
    ('en', 'auth.password.special', 'One special character'),
    ('en', 'auth.password.strength_weak', 'Weak'),
    ('en', 'auth.password.strength_fair', 'Fair'),
    ('en', 'auth.password.strength_good', 'Good'),
    
    -- Password strength indicator labels (German)
    ('de', 'auth.password.min_length', 'Mindestens {min} Zeichen'),
    ('de', 'auth.password.max_length', 'Maximal {max} Zeichen'),
    ('de', 'auth.password.uppercase', 'Ein Großbuchstabe'),
    ('de', 'auth.password.lowercase', 'Ein Kleinbuchstabe'),
    ('de', 'auth.password.digit', 'Eine Zahl'),
    ('de', 'auth.password.special', 'Ein Sonderzeichen'),
    ('de', 'auth.password.strength_weak', 'Schwach'),
    ('de', 'auth.password.strength_fair', 'Mittel'),
    ('de', 'auth.password.strength_good', 'Gut'),

    ('en', 'settings.title', 'Settings'),
    ('en', 'settings.description', 'Manage your account preferences and configuration.'),
    ('en', 'settings.back', 'Back'),
    ('en', 'settings.tabs.profile', 'Profile'),
    ('en', 'settings.tabs.providers', 'Providers'),
    ('en', 'settings.tabs.models', 'Models'),
    ('en', 'settings.tabs.ai_options', 'AI Options'),
    ('en', 'settings.tabs.usage_analytics', 'Usage Analytics'),
    ('en', 'settings.tabs.admin_users', 'Users'),
    ('en', 'settings.tabs.admin_config', 'Configuration'),
    ('en', 'settings.tabs.appearance', 'Appearance'),
    ('en', 'settings.admin_config.description', 'Application configuration settings'),
    ('en', 'settings.admin_config.coming_soon', 'Configuration management coming soon'),
    ('en', 'settings.admin_users.description', 'Manage user accounts and permissions'),
    ('en', 'settings.admin_users.coming_soon', 'User management coming soon'),
    ('en', 'settings.profile.title', 'Profile Information'),
    ('en', 'settings.profile.description', 'Your personal account information'),
    ('en', 'settings.profile.name', 'Name'),
    ('en', 'settings.profile.email', 'Email'),
    ('en', 'settings.profile.user_id', 'User ID'),
    ('en', 'settings.profile.edit', 'Edit'),
    ('en', 'settings.sessions.title', 'Active Sessions'),
    ('en', 'settings.sessions.description', 'Manage your active login sessions across devices'),
    ('en', 'settings.sessions.current', 'Current'),
    ('en', 'settings.sessions.sign_out', 'Sign Out'),
    ('en', 'settings.sessions.last_seen', 'Last seen'),
    ('en', 'settings.appearance.title', 'Appearance'),
    ('en', 'settings.appearance.description', 'Customize the look and feel'),
    ('en', 'settings.appearance.theme', 'Theme'),
    ('en', 'settings.appearance.theme_description', 'Choose between light and dark mode'),
    ('en', 'settings.appearance.theme_light', 'Light'),
    ('en', 'settings.appearance.theme_dark', 'Dark'),
    ('en', 'settings.appearance.theme_system', 'System'),
    ('en', 'settings.appearance.language', 'Language'),
    ('en', 'settings.appearance.language_description', 'Select your preferred language'),
    ('en', 'settings.appearance.language_en', 'English'),
    ('en', 'settings.appearance.language_de', 'German'),
    ('en', 'settings.appearance.themes', 'Themes'),
    ('en', 'settings.appearance.themes_description', 'Choose from a variety of beautiful themes'),
    ('en', 'settings.appearance.display_mode', 'Display Mode'),
    ('en', 'settings.appearance.display_mode_description', 'Select how content appears'),
    ('en', 'settings.appearance.built_in_themes', 'Built-in Themes'),
    ('en', 'settings.appearance.custom_themes', 'Custom Themes'),
    ('en', 'settings.appearance.no_themes_found', 'No themes found'),
    ('en', 'settings.appearance.try_different_search', 'Try a different search term'),
    ('en', 'settings.appearance.themes_powered_by', 'Themes powered by'),
    ('en', 'settings.appearance.search_themes', 'Search themes...'),
    ('en', 'settings.appearance.random_theme', 'Random theme'),
    ('en', 'settings.appearance.reset_theme', 'Reset theme'),
    ('en', 'settings.appearance.import_theme', 'Import Theme'),
    ('en', 'settings.appearance.loading_themes', 'Loading themes'),
    ('en', 'settings.theme_imported', 'Theme imported successfully'),
    ('en', 'settings.theme_deleted', 'Theme deleted'),
    ('en', 'settings.theme_saved', 'Theme preferences saved'),
    ('en', 'settings.import_theme', 'Import Theme'),
    ('en', 'settings.import_theme_description', 'Paste a theme URL from'),
    ('en', 'settings.theme_url', 'Theme URL'),
    ('en', 'settings.import', 'Import'),

    ('de', 'settings.appearance.themes', 'Themes'),
    ('de', 'settings.appearance.themes_description', 'Wählen Sie aus einer Vielzahl von schönen Themes'),
    ('de', 'settings.appearance.display_mode', 'Anzeigemodus'),
    ('de', 'settings.appearance.display_mode_description', 'Wählen Sie, wie der Inhalt angezeigt wird'),
    ('de', 'settings.appearance.built_in_themes', 'Integrierte Themes'),
    ('de', 'settings.appearance.custom_themes', 'Benutzerdefinierte Themes'),
    ('de', 'settings.appearance.no_themes_found', 'Keine Themes gefunden'),
    ('de', 'settings.appearance.try_different_search', 'Versuchen Sie einen anderen Suchbegriff'),
    ('de', 'settings.appearance.themes_powered_by', 'Themes bereitgestellt von'),
    ('de', 'settings.appearance.search_themes', 'Themes durchsuchen...'),
    ('de', 'settings.appearance.random_theme', 'Zufalliges Theme'),
    ('de', 'settings.appearance.reset_theme', 'Theme zurücksetzen'),
    ('de', 'settings.appearance.import_theme', 'Theme importieren'),
    ('de', 'settings.appearance.loading_themes', 'Themes werden geladen'),
    ('de', 'settings.theme_imported', 'Theme erfolgreich importiert'),
    ('de', 'settings.theme_deleted', 'Theme gelöscht'),
    ('de', 'settings.theme_saved', 'Theme-Einstellungen gespeichert'),
    ('de', 'settings.import_theme', 'Theme importieren'),
    ('de', 'settings.import_theme_description', 'Fügen Sie eine Theme-URL von'),
    ('de', 'settings.theme_url', 'Theme-URL'),
    ('de', 'settings.import', 'Importieren'),

    ('de', 'settings.title', 'Einstellungen'),
    ('de', 'settings.description', 'Verwalten Sie Ihre Kontoeinstellungen und Konfiguration.'),
    ('de', 'settings.back', 'Zurück'),
    ('de', 'settings.tabs.profile', 'Profil'),
    ('de', 'settings.tabs.providers', 'Anbieter'),
    ('de', 'settings.tabs.models', 'Modelle'),
    ('de', 'settings.tabs.ai_options', 'AI Optionen'),
    ('de', 'settings.tabs.usage_analytics', 'Benutzungsstatistiken'),
    ('de', 'settings.tabs.admin_users', 'Benutzer'),
    ('de', 'settings.tabs.admin_config', 'Konfiguration'),
    ('de', 'settings.tabs.appearance', 'Erscheinungsbild'),
    ('de', 'settings.admin_config.description', 'Anwendungskonfigurationseinstellungen'),
    ('de', 'settings.admin_config.coming_soon', 'Konfigurationsverwaltung kommt bald'),
    ('de', 'settings.admin_users.description', 'Benutzerkonten und Berechtigungen verwalten'),
    ('de', 'settings.admin_users.coming_soon', 'Benutzerverwaltung kommt bald'),
    ('de', 'settings.profile.title', 'Profilinformationen'),
    ('de', 'settings.profile.description', 'Ihre persönlichen Kontoinformationen'),
    ('de', 'settings.profile.name', 'Name'),
    ('de', 'settings.profile.email', 'E-Mail'),
    ('de', 'settings.profile.user_id', 'Benutzer-ID'),
    ('de', 'settings.profile.edit', 'Bearbeiten'),
    ('de', 'settings.sessions.title', 'Aktive Sitzungen'),
    ('de', 'settings.sessions.description', 'Verwalten Sie Ihre aktiven Anmeldesitzungen'),
    ('de', 'settings.sessions.current', 'Aktuell'),
    ('de', 'settings.sessions.sign_out', 'Abmelden'),
    ('de', 'settings.sessions.last_seen', 'Zuletzt gesehen'),
    ('de', 'settings.appearance.title', 'Erscheinungsbild'),
    ('de', 'settings.appearance.description', 'Passen Sie das Aussehen an'),
    ('de', 'settings.appearance.theme', 'Theme'),
    ('de', 'settings.appearance.theme_description', 'Wählen Sie zwischen Hell- und Dunkelmodus'),
    ('de', 'settings.appearance.theme_light', 'Hell'),
    ('de', 'settings.appearance.theme_dark', 'Dunkel'),
    ('de', 'settings.appearance.theme_system', 'System'),
    ('de', 'settings.appearance.language', 'Sprache'),
    ('de', 'settings.appearance.language_description', 'Wählen Sie Ihre bevorzugte Sprache'),
    ('de', 'settings.appearance.language_en', 'Englisch'),
    ('de', 'settings.appearance.language_de', 'Deutsch'),

    -- Providers
    ('en', 'settings.providers.title', 'BYOK AI Providers'),
    ('en', 'settings.providers.description', 'Bring your own API keys for enhanced AI capabilities'),
    ('en', 'settings.providers.edit', 'Configure'),
    ('en', 'settings.providers.configure', 'Configure'),
    ('en', 'settings.providers.configure_description', 'Enter your API credentials to enable this provider'),
    ('en', 'settings.providers.configured', 'Configured'),
    ('en', 'settings.providers.add_custom', 'Add Custom Provider'),
    ('en', 'settings.providers.name', 'Display Name'),
    ('en', 'settings.providers.api_key', 'API Key'),
    ('en', 'settings.providers.api_key_placeholder', 'Enter your API key'),
    ('en', 'settings.providers.api_key_hint', 'Your API key is stored securely. If encryption is configured, it is encrypted at rest'),
    ('en', 'settings.providers.base_url', 'Base URL'),
    ('en', 'settings.providers.enabled', 'Enable Provider'),
    ('en', 'settings.providers.enabled_hint', 'Allow this provider to be used for AI requests'),
    ('en', 'settings.providers.save_success', 'Provider configuration saved successfully'),
    ('en', 'settings.providers.save_error', 'Failed to save provider configuration'),
    ('en', 'settings.providers.delete_success', 'Provider removed successfully'),
    ('en', 'settings.providers.delete_error', 'Failed to remove provider'),
    ('en', 'settings.providers.openrouter_description', 'Access a wide variety of models through OpenRouter'),
    ('en', 'settings.providers.openai_description', 'Access GPT-5.2, GPT-5.2-Codex and other OpenAI models'),
    ('en', 'settings.providers.anthropic_description', 'Access Opus 4.5, Sonnet 4.5 and other Anthropic models'),
    ('en', 'settings.providers.google_description', 'Access Gemini 3 family and other Google AI models'),
    ('en', 'settings.providers.toggling_provider', 'Toggling provider'),
    ('en', 'settings.providers.toggling_provider_description', 'Please wait while the provider is being toggled'),
    ('en', 'settings.providers.toggling_provider_success', 'Provider successfully toggled'),
    ('en', 'settings.providers.syncing_provider', 'Syncing models'),
    ('en', 'settings.providers.syncing_provider_description', 'Models for this provider are being synced, this may take a moment'),
    ('en', 'settings.providers.syncing_provider_success', 'Models successfully synced'),

    ('de', 'settings.providers.title', 'BYOK AI-Anbieter'),
    ('de', 'settings.providers.description', 'Verwenden Sie Ihre eigenen API-Schlüssel für erweiterte AI-Funktionen'),
    ('de', 'settings.providers.edit', 'Konfigurieren'),
    ('de', 'settings.providers.configure', 'Konfigurieren'),
    ('de', 'settings.providers.configure_description', 'Geben Sie Ihre API-Anmeldedaten ein, um diesen Anbieter zu aktivieren'),
    ('de', 'settings.providers.configured', 'Konfiguriert'),
    ('de', 'settings.providers.add_custom', 'Benutzerdefinierten Anbieter hinzufügen'),
    ('de', 'settings.providers.name', 'Anzeigename'),
    ('de', 'settings.providers.api_key', 'API-Schlüssel'),
    ('de', 'settings.providers.api_key_placeholder', 'Geben Sie Ihren API-Schlüssel ein'),
    ('de', 'settings.providers.api_key_hint', 'Ihr API-Schlüssel wird sicher gespeichert. Wenn zudem die Verschlüsselung konfiguriert ist, wird er zudem verschlüsselt'),
    ('de', 'settings.providers.base_url', 'Basis-URL'),
    ('de', 'settings.providers.enabled', 'Anbieter aktivieren'),
    ('de', 'settings.providers.enabled_hint', 'Diesen Anbieter für AI-Anfragen verwenden'),
    ('de', 'settings.providers.save_success', 'Anbieterkonfiguration erfolgreich gespeichert'),
    ('de', 'settings.providers.save_error', 'Fehler beim Speichern der Anbieterkonfiguration'),
    ('de', 'settings.providers.delete_success', 'Anbieter erfolgreich entfernt'),
    ('de', 'settings.providers.delete_error', 'Fehler beim Entfernen des Anbieters'),
    ('de', 'settings.providers.openrouter_description', 'Greift auf eine Vielzahl von Modellen über OpenRouter zu'),
    ('de', 'settings.providers.openai_description', 'Greift auf GPT-5.2, GPT-5.2-Codex und andere OpenAI-Modelle zu'),
    ('de', 'settings.providers.anthropic_description', 'Greift auf Opus 4.5, Sonnet 4.5 und andere Anthropic-Modelle zu'),
    ('de', 'settings.providers.google_description', 'Greift auf Gemini 3 Familie und andere Google-Modelle zu'),
    ('de', 'settings.providers.toggling_provider', 'Anbieter umschalten'),
    ('de', 'settings.providers.toggling_provider_description', 'Bitte warten Sie, während der Anbieter umgeschaltet wird'),
    ('de', 'settings.providers.toggling_provider_success', 'Anbieter erfolgreich umgeschaltet'),
    ('de', 'settings.providers.syncing_provider', 'Synchronisiere Modelle'),
    ('de', 'settings.providers.syncing_provider_description', 'Modelle des Anbieters werden synchronisiert, dies kann einen Moment dauern'),
    ('de', 'settings.providers.syncing_provider_success', 'Modelle erfolgreich synchronisiert'),

    -- Chat Composer
    ('en', 'chat.composer.placeholder_default', 'Select a model and start typing...'),
    ('en', 'chat.composer.placeholder_model', 'Message {model}...'),
    ('en', 'chat.composer.hint', 'Press Enter to send, Shift+Enter for new line'),

    -- Chat Context Menu
    ('en', 'chat.context_menu.pin', 'Pin'),
    ('en', 'chat.context_menu.unpin', 'Unpin'),
    ('en', 'chat.context_menu.rename', 'Rename'),
    ('en', 'chat.context_menu.rename_prompt', 'Enter new title:'),
    ('en', 'chat.context_menu.move_to', 'Move to...'),
    ('en', 'chat.context_menu.archive', 'Archive'),
    ('en', 'chat.context_menu.unarchive', 'Unarchive'),
    ('en', 'chat.context_menu.export', 'Export'),
    ('en', 'chat.context_menu.delete', 'Delete'),
    ('en', 'chat.context_menu.delete_confirm', 'Are you sure you want to delete this chat? This cannot be undone.'),

    -- Chat List
    ('en', 'chat.list.new_chat', 'New chat'),
    ('en', 'chat.list.pinned', 'Pinned'),
    ('en', 'chat.list.today', 'Today'),
    ('en', 'chat.list.yesterday', 'Yesterday'),
    ('en', 'chat.list.past_7_days', 'Past 7 days'),
    ('en', 'chat.list.past_30_days', 'Past 30 days'),
    ('en', 'chat.list.older', 'Older'),

    -- Chat Empty State
    ('en', 'chat.empty_state.username_default', 'there'),
    ('en', 'chat.empty_state.greeting_morning', 'Good morning'),
    ('en', 'chat.empty_state.greeting_afternoon', 'Good afternoon'),
    ('en', 'chat.empty_state.greeting_evening', 'Good evening'),
    ('en', 'chat.empty_state.desc_1', 'Ask me anything about code, math, or creative writing.'),
    ('en', 'chat.empty_state.desc_2', 'I can help you brainstorm, analyze data, or write content.'),
    ('en', 'chat.empty_state.desc_3', 'Need help with a project? Just describe what you''re working on.'),
    ('en', 'chat.empty_state.desc_4', 'I''m here to assist with research, explanations, or problem-solving.'),
    ('en', 'chat.empty_state.desc_5', 'Start a conversation and let''s explore ideas together.'),

    -- Code Preview
    ('en', 'chat.code_preview.title', 'Preview:'),
    ('en', 'chat.code_preview.preview', 'Preview'),
    ('en', 'chat.code_preview.code', 'Code'),
    ('en', 'chat.code_preview.copied', 'Copied'),
    ('en', 'chat.code_preview.copy', 'Copy'),
    ('en', 'chat.code_preview.open_in_tab', 'Open in Tab'),
    ('en', 'chat.code_preview.close', 'Close'),
    ('en', 'chat.code_preview.lines', '{count} lines'),

    -- Context Indicator
    ('en', 'chat.context_indicator.usage', 'Context Usage'),
    ('en', 'chat.context_indicator.tokens', 'tokens'),
    ('en', 'chat.context_indicator.approaching_limit', 'Approaching context limit'),

    -- Message Actions
    ('en', 'chat.message_actions.copied', 'Copied!'),
    ('en', 'chat.message_actions.copy_message', 'Copy message'),
    ('en', 'chat.message_actions.regenerate', 'Regenerate response'),
    ('en', 'chat.message_actions.details', 'Message Details'),
    ('en', 'chat.message_actions.model', 'Model'),
    ('en', 'chat.message_actions.input_tokens', 'Input tokens'),
    ('en', 'chat.message_actions.output_tokens', 'Output tokens'),
    ('en', 'chat.message_actions.reasoning_tokens', 'Reasoning tokens'),
    ('en', 'chat.message_actions.input_cost', 'Input cost'),
    ('en', 'chat.message_actions.output_cost', 'Output cost'),
    ('en', 'chat.message_actions.reasoning_cost', 'Reasoning cost'),
    ('en', 'chat.message_actions.total_cost', 'Total cost'),
    ('en', 'chat.message_actions.response_latency', 'Response latency'),
    ('en', 'chat.message_actions.reasoning_latency', 'Reasoning latency'),
    ('en', 'chat.message_actions.created', 'Created'),
    ('en', 'chat.message_actions.branch_to_new_chat', 'Branch to new chat'),

    -- Model Selector
    ('en', 'chat.model_selector.search_models', 'Search models...'),
    ('en', 'chat.model_selector.select_model', 'Select model'),
    ('en', 'chat.model_selector.no_models', 'No models found'),
    ('en', 'chat.model_selector.tool_calling', 'Tool calling'),
    ('en', 'chat.model_selector.reasoning_capable', 'Reasoning capable'),
    ('en', 'chat.model_selector.toggle_favorite', 'Toggle favorite'),

    -- Reasoning Selector
    ('en', 'chat.reasoning_selector.disabled', 'Disabled'),
    ('en', 'chat.reasoning_selector.token_limit', 'Token Limit'),
    ('en', 'chat.reasoning_selector.tokens', 'Tokens'),
    ('en', 'chat.reasoning_selector.none', 'None'),
    ('en', 'chat.reasoning_selector.minimal', 'Minimal'),
    ('en', 'chat.reasoning_selector.low', 'Low'),
    ('en', 'chat.reasoning_selector.medium', 'Medium'),
    ('en', 'chat.reasoning_selector.high', 'High'),
    ('en', 'chat.reasoning_selector.extra_high', 'Extra High'),

    -- Tool Selector
    ('en', 'chat.tool_selector.search_tools', 'Search tools...'),
    ('en', 'chat.tool_selector.no_tool_support', 'Selected model doesn''t support tools'),
    ('en', 'chat.tool_selector.no_tools_found', 'No tools found'),

    -- Streaming Message
    ('en', 'chat.streaming_message.assistant', 'Assistant'),

    -- German translations
    -- Chat Composer
    ('de', 'chat.composer.placeholder_default', 'Wählen Sie ein Modell und beginnen Sie zu tippen...'),
    ('de', 'chat.composer.placeholder_model', 'Nachricht an {model}...'),
    ('de', 'chat.composer.hint', 'Drücken Sie Enter zum Senden, Shift+Enter für eine neue Zeile'),

    -- Chat Context Menu
    ('de', 'chat.context_menu.pin', 'Anheften'),
    ('de', 'chat.context_menu.unpin', 'Lösen'),
    ('de', 'chat.context_menu.rename', 'Umbenennen'),
    ('de', 'chat.context_menu.rename_prompt', 'Neuen Titel eingeben:'),
    ('de', 'chat.context_menu.move_to', 'Verschieben nach...'),
    ('de', 'chat.context_menu.archive', 'Archivieren'),
    ('de', 'chat.context_menu.unarchive', 'Aus Archiv entnehmen'),
    ('de', 'chat.context_menu.export', 'Exportieren'),
    ('de', 'chat.context_menu.delete', 'Löschen'),
    ('de', 'chat.context_menu.delete_confirm', 'Möchten Sie diesen Chat wirklich löschen? Dies kann nicht rückgängig gemacht werden.'),

    -- Chat List
    ('de', 'chat.list.new_chat', 'Neuer Chat'),
    ('de', 'chat.list.pinned', 'Angeheftet'),
    ('de', 'chat.list.today', 'Heute'),
    ('de', 'chat.list.yesterday', 'Gestern'),
    ('de', 'chat.list.past_7_days', 'Letzte 7 Tage'),
    ('de', 'chat.list.past_30_days', 'Letzte 30 Tage'),
    ('de', 'chat.list.older', 'Älter'),

    -- Chat Empty State
    ('de', 'chat.empty_state.username_default', 'dort'),
    ('de', 'chat.empty_state.greeting_morning', 'Guten Morgen'),
    ('de', 'chat.empty_state.greeting_afternoon', 'Guten Tag'),
    ('de', 'chat.empty_state.greeting_evening', 'Guten Abend'),
    ('de', 'chat.empty_state.desc_1', 'Fragen Sie mich alles über Code, Mathematik oder kreatives Schreiben.'),
    ('de', 'chat.empty_state.desc_2', 'Ich helfe Ihnen beim Brainstorming, der Datenanalyse oder beim Schreiben von Inhalten.'),
    ('de', 'chat.empty_state.desc_3', 'Brauchen Sie Hilfe bei einem Projekt? Beschreiben Sie einfach, woran Sie arbeiten.'),
    ('de', 'chat.empty_state.desc_4', 'Ich bin hier, um bei Recherchen, Erklärungen oder Problemlösungen zu helfen.'),
    ('de', 'chat.empty_state.desc_5', 'Beginnen Sie ein Gespräch und lass uns Ideen gemeinsam erkunden.'),

    -- Code Preview
    ('de', 'chat.code_preview.title', 'Vorschau:'),
    ('de', 'chat.code_preview.preview', 'Vorschau'),
    ('de', 'chat.code_preview.code', 'Code'),
    ('de', 'chat.code_preview.copied', 'Kopiert!'),
    ('de', 'chat.code_preview.copy', 'Kopieren'),
    ('de', 'chat.code_preview.open_in_tab', 'In Tab öffnen'),
    ('de', 'chat.code_preview.close', 'Schließen'),
    ('de', 'chat.code_preview.lines', '{count} Zeilen'),

    -- Context Indicator
    ('de', 'chat.context_indicator.usage', 'Kontextnutzung'),
    ('de', 'chat.context_indicator.tokens', 'Tokens'),
    ('de', 'chat.context_indicator.approaching_limit', 'Kontextlimit nähert sich'),

    -- Message Actions
    ('de', 'chat.message_actions.copied', 'Kopiert!'),
    ('de', 'chat.message_actions.copy_message', 'Nachricht kopieren'),
    ('de', 'chat.message_actions.regenerate', 'Antwort neu generieren'),
    ('de', 'chat.message_actions.details', 'Nachrichtendetails'),
    ('de', 'chat.message_actions.model', 'Modell'),
    ('de', 'chat.message_actions.input_tokens', 'Eingabe-Tokens'),
    ('de', 'chat.message_actions.output_tokens', 'Ausgabe-Tokens'),
    ('de', 'chat.message_actions.reasoning_tokens', 'Reasoning-Tokens'),
    ('de', 'chat.message_actions.input_cost', 'Eingabekosten'),
    ('de', 'chat.message_actions.output_cost', 'Ausgabekosten'),
    ('de', 'chat.message_actions.reasoning_cost', 'Reasoning-Kosten'),
    ('de', 'chat.message_actions.total_cost', 'Gesamtkosten'),
    ('de', 'chat.message_actions.response_latency', 'Antwortlatenz'),
    ('de', 'chat.message_actions.reasoning_latency', 'Reasoning-Latenz'),
    ('de', 'chat.message_actions.created', 'Erstellt'),
    ('de', 'chat.message_actions.branch_to_new_chat', 'Neuen Chat abzweigen'),

    -- Model Selector
    ('de', 'chat.model_selector.search_models', 'Modelle suchen...'),
    ('de', 'chat.model_selector.select_model', 'Modell auswählen'),
    ('de', 'chat.model_selector.no_models', 'Keine Modelle gefunden'),
    ('de', 'chat.model_selector.tool_calling', 'Tool-Aufruf'),
    ('de', 'chat.model_selector.reasoning_capable', 'Reasoning-fähig'),
    ('de', 'chat.model_selector.toggle_favorite', 'Favorit umschalten'),

    -- Reasoning Selector
    ('de', 'chat.reasoning_selector.disabled', 'Deaktiviert'),
    ('de', 'chat.reasoning_selector.token_limit', 'Token-Limit'),
    ('de', 'chat.reasoning_selector.tokens', 'Tokens'),
    ('de', 'chat.reasoning_selector.none', 'Keine'),
    ('de', 'chat.reasoning_selector.minimal', 'Minimal'),
    ('de', 'chat.reasoning_selector.low', 'Niedrig'),
    ('de', 'chat.reasoning_selector.medium', 'Mittel'),
    ('de', 'chat.reasoning_selector.high', 'Hoch'),
    ('de', 'chat.reasoning_selector.extra_high', 'Sehr Hoch'),

    -- Tool Selector
    ('de', 'chat.tool_selector.search_tools', 'Tools suchen...'),
    ('de', 'chat.tool_selector.no_tool_support', 'Ausgewähltes Modell unterstützt keine Tools'),
    ('de', 'chat.tool_selector.no_tools_found', 'Keine Tools gefunden'),

    -- Streaming Message
    ('de', 'chat.streaming_message.assistant', 'Assistent'),

    -- Message Item
    ('en', 'chat.message_item.reasoning', 'Reasoning'),
    ('en', 'chat.message_item.edit_message', 'Edit message'),
    ('en', 'chat.message_item.copy_message', 'Copy message'),
    ('en', 'chat.message_item.copied', 'Copied'),
    ('en', 'chat.message_item.cancel', 'Cancel'),
    ('en', 'chat.message_item.save_and_fork', 'Save & Fork'),
    ('en', 'chat.message_item.branch_to_new_chat', 'Branch to new chat'),
    ('de', 'chat.message_item.reasoning', 'Reasoning'),
    ('de', 'chat.message_item.edit_message', 'Nachricht bearbeiten'),
    ('de', 'chat.message_item.copy_message', 'Nachricht kopieren'),
    ('de', 'chat.message_item.copied', 'Kopiert'),
    ('de', 'chat.message_item.cancel', 'Abbrechen'),
    ('de', 'chat.message_item.save_and_fork', 'Speichern & abzweigen'),
    ('de', 'chat.message_item.branch_to_new_chat', 'Neuen Chat abzweigen'),

    -- Tool Execution Display
    ('en', 'chat.tool_execution.tool', 'Tool'),
    ('en', 'chat.tool_execution.running', 'Running...'),
    ('en', 'chat.tool_execution.failed', 'Failed'),
    ('en', 'chat.tool_execution.complete', 'Complete'),
    ('en', 'chat.tool_execution.arguments', 'Arguments'),
    ('en', 'chat.tool_execution.output', 'Output'),
    ('en', 'chat.tool_execution.error', 'Error'),
    ('en', 'chat.tool_execution.completed_in', 'Completed in {ms}ms'),

    ('de', 'chat.tool_execution.tool', 'Tool'),
    ('de', 'chat.tool_execution.running', 'Wird ausgeführt...'),
    ('de', 'chat.tool_execution.failed', 'Fehlgeschlagen'),
    ('de', 'chat.tool_execution.complete', 'Abgeschlossen'),
    ('de', 'chat.tool_execution.arguments', 'Argumente'),
    ('de', 'chat.tool_execution.output', 'Ausgabe'),
    ('de', 'chat.tool_execution.error', 'Fehler'),
    ('de', 'chat.tool_execution.completed_in', 'Abgeschlossen in {ms}ms'),

    -- Image Generation
    ('en', 'chat.image_preview.download', 'Download'),
    ('en', 'chat.image_preview.copy', 'Copy URL'),
    ('en', 'chat.image_preview.copied', 'Copied!'),
    ('en', 'chat.tool_execution.generated_image', 'Generated Image'),
    
    ('de', 'chat.image_preview.download', 'Herunterladen'),
    ('de', 'chat.image_preview.copy', 'URL kopieren'),
    ('de', 'chat.image_preview.copied', 'Kopiert!'),
    ('de', 'chat.tool_execution.generated_image', 'Generiertes Bild'),


    -- Schema Builder
    ('en', 'settings.schema_builder.type', 'Type'),
    ('en', 'settings.schema_builder.default', 'Default'),
    ('en', 'settings.schema_builder.default_placeholder', 'Default value'),
    ('en', 'settings.schema_builder.description', 'Description'),
    ('en', 'settings.schema_builder.description_placeholder', 'Describe this parameter'),
    ('en', 'settings.schema_builder.options', 'Options (for dropdown)'),
    ('en', 'settings.schema_builder.options_placeholder', 'option1, option2, option3'),
    ('en', 'settings.schema_builder.options_hint', 'Comma-separated. Leave empty for free-form input.'),
    ('en', 'settings.schema_builder.add_property', 'Add Property'),
    ('en', 'settings.schema_builder.required', 'Required'),
    ('en', 'settings.schema_builder.secret', 'Secret (e.g. API key)'),
    ('en', 'settings.schema_builder.json_format', 'JSON Schema format'),

    ('de', 'settings.schema_builder.type', 'Typ'),
    ('de', 'settings.schema_builder.default', 'Standard'),
    ('de', 'settings.schema_builder.default_placeholder', 'Standardwert'),
    ('de', 'settings.schema_builder.description', 'Beschreibung'),
    ('de', 'settings.schema_builder.description_placeholder', 'Beschreiben Sie diesen Parameter'),
    ('de', 'settings.schema_builder.options', 'Optionen (für Dropdown)'),
    ('de', 'settings.schema_builder.options_placeholder', 'Option1, Option2, Option3'),
    ('de', 'settings.schema_builder.options_hint', 'Kommagetrennt. Leer lassen für freie Eingabe.'),
    ('de', 'settings.schema_builder.add_property', 'Eigenschaft hinzufügen'),
    ('de', 'settings.schema_builder.required', 'Erforderlich'),
    ('de', 'settings.schema_builder.secret', 'Geheim (z.B. API-Schlüssel)'),
    ('de', 'settings.schema_builder.json_format', 'JSON-Schema-Format'),

    -- Tool Test Dialog
    ('en', 'settings.tool_test.test', 'Test'),
    ('en', 'settings.tool_test.description', 'Enter input values to test this tool'),
    ('en', 'settings.tool_test.select_function', 'Select Function'),
    ('en', 'settings.tool_test.select_placeholder', 'Select a function...'),
    ('en', 'settings.tool_test.form', 'Form'),
    ('en', 'settings.tool_test.no_parameters', 'This tool has no input parameters'),
    ('en', 'settings.tool_test.array_placeholder', 'Comma-separated values'),
    ('en', 'settings.tool_test.array_hint', 'Enter values separated by commas'),
    ('en', 'settings.tool_test.success', 'Success'),
    ('en', 'settings.tool_test.failed', 'Failed'),
    ('en', 'settings.tool_test.view_output', 'View output'),
    ('en', 'settings.tool_test.run_test', 'Run Test'),

    ('de', 'settings.tool_test.test', 'Testen'),
    ('de', 'settings.tool_test.description', 'Geben Sie Eingabewerte ein, um dieses Tool zu testen'),
    ('de', 'settings.tool_test.select_function', 'Funktion auswählen'),
    ('de', 'settings.tool_test.select_placeholder', 'Wählen Sie eine Funktion...'),
    ('de', 'settings.tool_test.form', 'Formular'),
    ('de', 'settings.tool_test.no_parameters', 'Dieses Tool hat keine Eingabeparameter'),
    ('de', 'settings.tool_test.array_placeholder', 'Kommagetrennte Werte'),
    ('de', 'settings.tool_test.array_hint', 'Geben Sie Werte durch Kommas getrennt ein'),
    ('de', 'settings.tool_test.success', 'Erfolgreich'),
    ('de', 'settings.tool_test.failed', 'Fehlgeschlagen'),
    ('de', 'settings.tool_test.view_output', 'Ausgabe anzeigen'),
    ('de', 'settings.tool_test.run_test', 'Test ausführen'),

    -- Tools Settings
    ('en', 'settings.tabs.tools', 'Tools'),
    ('en', 'settings.tools.title', 'Tools'),
    ('en', 'settings.tools.description', 'Manage custom tools that AI models can use during conversations'),
    ('en', 'settings.tools.add', 'Add Tool'),
    ('en', 'settings.tools.no_tools', 'No tools yet'),
    ('en', 'settings.tools.no_tools_description', 'Create custom tools to extend AI capabilities'),
    ('en', 'settings.tools.create_first', 'Create your first tool'),
    ('en', 'settings.tools.template', 'Template'),
    ('en', 'settings.tools.configured', 'Configured'),
    ('en', 'settings.tools.no_description', 'No description'),
    ('en', 'settings.tools.configure', 'Configure'),
    ('en', 'settings.tools.edit', 'Edit Tool'),
    ('en', 'settings.tools.create', 'Create Tool'),
    ('en', 'settings.tools.general', 'General'),
    ('en', 'settings.tools.source', 'Source'),
    ('en', 'settings.tools.functions', 'Functions'),
    ('en', 'settings.tools.identifier', 'Identifier'),
    ('en', 'settings.tools.identifier_placeholder', 'fetch_website'),
    ('en', 'settings.tools.display_name', 'Display Name'),
    ('en', 'settings.tools.display_name_placeholder', 'Fetch Website'),
    ('en', 'settings.tools.description', 'Description'),
    ('en', 'settings.tools.description_placeholder', 'Fetches content from a URL'),
    ('en', 'settings.tools.soon', 'Soon'),
    ('en', 'settings.tools.url_placeholder', 'https://api.example.com/{{input.query}}'),
    ('en', 'settings.tools.headers', 'Headers (JSON)'),
    ('en', 'settings.tools.headers_placeholder', '{"Authorization": "Bearer {{settings.api_key}}"}'),
    ('en', 'settings.tools.stdio', 'Stdio (Local Process)'),
    ('en', 'settings.tools.sse', 'SSE (HTTP)'),
    ('en', 'settings.tools.command_placeholder', 'Command (e.g. npx)'),
    ('en', 'settings.tools.args_placeholder', 'Arguments (comma-separated)'),
    ('en', 'settings.tools.tool_name_placeholder', 'Tool name from server'),
    ('en', 'settings.tools.settings_schema', 'Settings Schema (optional)'),
    ('en', 'settings.tools.new', 'New'),
    ('en', 'settings.tools.name', 'Name'),
    ('en', 'settings.tools.function_name_placeholder', 'search_web'),
    ('en', 'settings.tools.entrypoint', 'Entrypoint'),
    ('en', 'settings.tools.optional', 'Optional'),
    ('en', 'settings.tools.function_description_placeholder', 'What this function does'),
    ('en', 'settings.tools.input_schema', 'Input Schema'),
    ('en', 'settings.tools.remove', 'Remove'),
    ('en', 'settings.tools.settings', 'Tool Settings'),
    ('en', 'settings.tools.settings_description', 'Configure your personal settings for {name}'),
    ('en', 'settings.tools.select_placeholder', 'Select...'),

    ('de', 'settings.tabs.tools', 'Tools'),
    ('de', 'settings.tools.title', 'Tools'),
    ('de', 'settings.tools.description', 'Verwalten Sie benutzerdefinierte Tools, die KI-Modelle während Gesprächen verwenden können'),
    ('de', 'settings.tools.add', 'Tool hinzufügen'),
    ('de', 'settings.tools.no_tools', 'Noch keine Tools'),
    ('de', 'settings.tools.no_tools_description', 'Erstellen Sie benutzerdefinierte Tools, um die KI-Fähigkeiten zu erweitern'),
    ('de', 'settings.tools.create_first', 'Erstellen Sie Ihren ersten Tool'),
    ('de', 'settings.tools.template', 'Vorlage'),
    ('de', 'settings.tools.configured', 'Konfiguriert'),
    ('de', 'settings.tools.no_description', 'Keine Beschreibung'),
    ('de', 'settings.tools.configure', 'Konfigurieren'),
    ('de', 'settings.tools.edit', 'Tool bearbeiten'),
    ('de', 'settings.tools.create', 'Tool erstellen'),
    ('de', 'settings.tools.general', 'Allgemein'),
    ('de', 'settings.tools.source', 'Quelle'),
    ('de', 'settings.tools.functions', 'Funktionen'),
    ('de', 'settings.tools.identifier', 'Bezeichner'),
    ('de', 'settings.tools.identifier_placeholder', 'website_aufrufen'),
    ('de', 'settings.tools.display_name', 'Anzeigename'),
    ('de', 'settings.tools.display_name_placeholder', 'Website abrufen'),
    ('de', 'settings.tools.description', 'Beschreibung'),
    ('de', 'settings.tools.description_placeholder', 'Ruft Inhalte von einer URL ab'),
    ('de', 'settings.tools.soon', 'Bald'),
    ('de', 'settings.tools.url_placeholder', 'https://api.example.com/{{input.query}}'),
    ('de', 'settings.tools.headers', 'Header (JSON)'),
    ('de', 'settings.tools.headers_placeholder', '{"Authorization": "Bearer {{settings.api_key}}"}'),
    ('de', 'settings.tools.stdio', 'Stdio (Lokaler Prozess)'),
    ('de', 'settings.tools.sse', 'SSE (HTTP)'),
    ('de', 'settings.tools.command_placeholder', 'Befehl (z.B. npx)'),
    ('de', 'settings.tools.args_placeholder', 'Argumente (kommagetrennt)'),
    ('de', 'settings.tools.tool_name_placeholder', 'Tool-Name vom Server'),
    ('de', 'settings.tools.settings_schema', 'Einstellungsschema (optional)'),
    ('de', 'settings.tools.new', 'Neu'),
    ('de', 'settings.tools.name', 'Name'),
    ('de', 'settings.tools.function_name_placeholder', 'web_suchen'),
    ('de', 'settings.tools.entrypoint', 'Einstiegspunkt'),
    ('de', 'settings.tools.optional', 'Optional'),
    ('de', 'settings.tools.function_description_placeholder', 'Was diese Funktion tut'),
    ('de', 'settings.tools.input_schema', 'Eingabeschema'),
    ('de', 'settings.tools.remove', 'Entfernen'),
    ('de', 'settings.tools.settings', 'Tool-Einstellungen'),
    ('de', 'settings.tools.settings_description', 'Konfigurieren Sie Ihre persönlichen Einstellungen für {name}'),
    ('de', 'settings.tools.select_placeholder', 'Auswählen...'),

        -- Display Mode section (English)
    ('en', 'settings.appearance.display_mode', 'Display Mode'),
    ('en', 'settings.appearance.display_mode_description', 'Choose between light and dark mode'),
    
    -- Themes section (English)
    ('en', 'settings.appearance.themes', 'Themes'),
    ('en', 'settings.appearance.themes_description', 'Select and manage your color themes'),
    ('en', 'settings.appearance.search_themes', 'Search themes...'),
    ('en', 'settings.appearance.random_theme', 'Random theme'),
    ('en', 'settings.appearance.reset_theme', 'Reset theme'),
    ('en', 'settings.appearance.import_theme', 'Import Theme'),
    ('en', 'settings.appearance.loading_themes', 'Loading themes...'),
    ('en', 'settings.appearance.custom_themes', 'My Themes'),
    ('en', 'settings.appearance.built_in_themes', 'Built-in Themes'),
    ('en', 'settings.appearance.no_themes_found', 'No themes found'),
    ('en', 'settings.appearance.try_different_search', 'Try adjusting your search query'),
    ('en', 'settings.appearance.themes_powered_by', 'Get more themes at'),
    ('en', 'settings.theme_deleted', 'Theme deleted'),
    ('en', 'settings.theme_saved', 'Theme preferences saved'),

    -- Display Mode section (German)
    ('de', 'settings.appearance.display_mode', 'Anzeigemodus'),
    ('de', 'settings.appearance.display_mode_description', 'Wählen Sie zwischen Hell- und Dunkelmodus'),
    
    -- Themes section (German)
    ('de', 'settings.appearance.themes', 'Themes'),
    ('de', 'settings.appearance.themes_description', 'Farbthemen auswählen und verwalten'),
    ('de', 'settings.appearance.search_themes', 'Themes suchen...'),
    ('de', 'settings.appearance.random_theme', 'Zufälliges Theme'),
    ('de', 'settings.appearance.reset_theme', 'Theme zurücksetzen'),
    ('de', 'settings.appearance.import_theme', 'Theme importieren'),
    ('de', 'settings.appearance.loading_themes', 'Themes werden geladen...'),
    ('de', 'settings.appearance.custom_themes', 'Meine Themes'),
    ('de', 'settings.appearance.built_in_themes', 'Eingebaute Themes'),
    ('de', 'settings.appearance.no_themes_found', 'Keine Themes gefunden'),
    ('de', 'settings.appearance.try_different_search', 'Versuchen Sie eine andere Suchanfrage'),
    ('de', 'settings.appearance.themes_powered_by', 'Mehr Themes bei'),
    ('de', 'settings.theme_deleted', 'Theme gelöscht'),
    ('de', 'settings.theme_saved', 'Theme-Einstellungen gespeichert'),

    -- Admin Users
    ('en', 'settings.admin_users.create_user', 'Create User'),
    ('de', 'settings.admin_users.create_user', 'Benutzer erstellen'),
    ('en', 'settings.admin_users.search_placeholder', 'Search users...'),
    ('de', 'settings.admin_users.search_placeholder', 'Benutzer suchen...'),
    ('en', 'settings.admin_users.filter_role', 'Filter role'),
    ('de', 'settings.admin_users.filter_role', 'Nach Rolle filtern'),
    ('en', 'settings.admin_users.role_all', 'All'),
    ('de', 'settings.admin_users.role_all', 'Alle'),
    ('en', 'settings.admin_users.no_users', 'No users found'),
    ('de', 'settings.admin_users.no_users', 'Keine Benutzer gefunden'),
    ('en', 'settings.admin_users.cannot_modify_self', 'You cannot modify your own account'),
    ('de', 'settings.admin_users.cannot_modify_self', 'Sie können Ihr eigenes Konto nicht bearbeiten'),
    ('en', 'settings.admin_users.create_user_description', 'Create a new user with optional roles and password'),
    ('de', 'settings.admin_users.create_user_description', 'Erstellen Sie einen neuen Benutzer mit optionalen Rollen und Passwort'),
    ('en', 'settings.admin_users.field_email', 'Email'),
    ('de', 'settings.admin_users.field_email', 'E-Mail'),
    ('en', 'settings.admin_users.field_username', 'Username'),
    ('de', 'settings.admin_users.field_username', 'Benutzername'),
    ('en', 'settings.admin_users.field_password', 'Password'),
    ('de', 'settings.admin_users.field_password', 'Passwort'),
    ('en', 'settings.admin_users.field_roles', 'Roles'),
    ('de', 'settings.admin_users.field_roles', 'Rollen'),
    ('en', 'settings.admin_users.edit_user', 'Edit User'),
    ('de', 'settings.admin_users.edit_user', 'Benutzer bearbeiten'),
    ('en', 'settings.admin_users.reset_password', 'Reset Password'),
    ('de', 'settings.admin_users.reset_password', 'Passwort zurücksetzen'),
    ('en', 'settings.admin_users.reset_password_placeholder', 'New password (leave blank to keep current)'),
    ('de', 'settings.admin_users.reset_password_placeholder', 'Neues Passwort (leer lassen, um aktuelles zu behalten)'),
    ('en', 'settings.admin_users.delete_user', 'Delete User'),
    ('de', 'settings.admin_users.delete_user', 'Benutzer löschen'),
    ('en', 'settings.admin_users.delete_confirm', 'Are you sure you want to delete {username}? This action cannot be undone.'),
    ('de', 'settings.admin_users.delete_confirm', 'Möchten Sie {username} wirklich löschen? Diese Aktion kann nicht rückgängig gemacht werden.'),
    ('en', 'settings.admin_users.load_error', 'Failed to load users'),
    ('de', 'settings.admin_users.load_error', 'Benutzer konnten nicht geladen werden'),
    ('en', 'settings.admin_users.create_success', 'User created'),
    ('de', 'settings.admin_users.create_success', 'Benutzer erstellt'),
    ('en', 'settings.admin_users.create_error', 'Failed to create user'),
    ('de', 'settings.admin_users.create_error', 'Fehler beim Erstellen des Benutzers'),
    ('en', 'settings.admin_users.edit_success', 'User updated'),
    ('de', 'settings.admin_users.edit_success', 'Benutzer aktualisiert'),
    ('en', 'settings.admin_users.edit_error', 'Failed to update user'),
    ('de', 'settings.admin_users.edit_error', 'Fehler beim Aktualisieren des Benutzers'),
    ('en', 'settings.admin_users.delete_success', 'User deleted'),
    ('de', 'settings.admin_users.delete_success', 'Benutzer gelöscht'),
    ('en', 'settings.admin_users.delete_error', 'Failed to delete user'),
    ('de', 'settings.admin_users.delete_error', 'Fehler beim Löschen des Benutzers'),
    ('en', 'settings.admin_users.select_roles', 'Select roles'),
    ('de', 'settings.admin_users.select_roles', 'Rollen auswählen'),

    -- models
    ('en', 'settings.tabs.models',  'Models'),
    ('en', 'settings.models.description', 'Manage and configure available AI models.'),
    ('en', 'settings.models.search', 'Search models...'),
    ('en', 'settings.models.no_models', 'No models found.'),
    ('en', 'settings.models.not_found', 'Model not found.'),
    ('en', 'settings.models.save_success', 'Model configuration saved successfully.'),
    ('en', 'settings.models.save_error', 'Failed to save model configuration.'),
    ('en', 'settings.models.editor.loading', 'Loading model...'),
    ('en', 'settings.models.editor.general', 'General Configuration'),
    ('en', 'settings.models.editor.display_name', 'Display Name'),
    ('en', 'settings.models.editor.is_enabled', 'Enabled'),
    ('en', 'settings.models.editor.description', 'Description'),
    ('en', 'settings.models.editor.icon_url', 'Icon URL'),
    ('en', 'settings.models.editor.system_prompt', 'System Prompt'),
    ('en', 'settings.models.editor.system_prompt_desc', 'Base system prompt applied to all conversations with this model.'),
    ('en', 'settings.models.editor.system_prompt_placeholder', 'You are a helpful AI assistant...'),
    ('en', 'settings.models.editor.sampling', 'Sampling Parameters'),
    ('en', 'settings.models.editor.sampling_desc', 'Default sampling parameters for this model.'),
    ('en', 'settings.models.editor.temperature', 'Temperature'),
    ('en', 'settings.models.editor.top_p', 'Top P'),
    ('en', 'settings.models.editor.max_tokens', 'Max Tokens'),
    ('en', 'settings.models.editor.icon_tab_url', 'URL'),
    ('en', 'settings.models.editor.icon_tab_upload', 'Upload'),
    ('en', 'settings.models.editor.icon_choose_file', 'Choose Image'),
    ('en', 'settings.models.editor.icon_upload_hint', 'Upload a local image to use as the model icon.'),
    ('en', 'settings.models.editor.icon_clear', 'Clear icon'),
    ('en', 'settings.models.editor.description_tab_write', 'Write'),
    ('en', 'settings.models.editor.description_tab_preview', 'Preview'),
    ('en', 'settings.models.editor.description_placeholder', 'Describe this model using markdown...'),
    ('en', 'settings.models.editor.description_markdown_hint', 'Markdown is supported'),
    ('en', 'settings.models.editor.description_empty_preview', 'Nothing to preview'),
    ('en', 'settings.models.editor.unsaved_changes', 'Unsaved changes'),
    ('en', 'settings.models.editor.unsaved_dialog_title', 'Unsaved Changes'),
    ('en', 'settings.models.editor.unsaved_dialog_desc', 'You have unsaved changes that will be lost. Are you sure you want to go back?'),
    ('en', 'settings.models.editor.discard_changes', 'Discard & Go Back'),
    ('en', 'settings.models.editor.keep_editing', 'Keep Editing'),
    ('de', 'settings.models.editor.icon_tab_url', 'URL'),
    ('de', 'settings.models.editor.icon_tab_upload', 'Hochladen'),
    ('de', 'settings.models.editor.icon_choose_file', 'Bild auswählen'),
    ('de', 'settings.models.editor.icon_upload_hint', 'Lade ein lokales Bild als Modell-Icon hoch.'),
    ('de', 'settings.models.editor.icon_clear', 'Icon entfernen'),
    ('de', 'settings.models.editor.description_tab_write', 'Schreiben'),
    ('de', 'settings.models.editor.description_tab_preview', 'Vorschau'),
    ('de', 'settings.models.editor.description_placeholder', 'Beschreibe dieses Modell mit Markdown...'),
    ('de', 'settings.models.editor.description_markdown_hint', 'Markdown wird unterstützt'),
    ('de', 'settings.models.editor.description_empty_preview', 'Nichts zum Anzeigen'),
    ('de', 'settings.models.editor.unsaved_changes', 'Nicht gespeicherte Änderungen'),
    ('de', 'settings.models.editor.unsaved_dialog_title', 'Nicht gespeicherte Änderungen'),
    ('de', 'settings.models.editor.unsaved_dialog_desc', 'Du hast nicht gespeicherte Änderungen, die verloren gehen. Möchtest du wirklich zurückgehen?'),
    ('de', 'settings.models.editor.discard_changes', 'Verwerfen & Zurück'),
    ('de', 'settings.models.editor.keep_editing', 'Weiter bearbeiten')

    ON CONFLICT (language, key_path) DO NOTHING;
