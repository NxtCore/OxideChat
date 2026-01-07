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
    ('language', 'en')
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

CREATE TYPE provider_kind AS ENUM (
    'OPENAI',
    'OPENAI_COMPAT',
    'OPENROUTER',
    'ANTHROPIC',
    'GOOGLE',
    'CUSTOM'
);

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
    is_hidden BOOLEAN DEFAULT false,
    
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
    role VARCHAR(20) NOT NULL,
    content TEXT NOT NULL,
    reasoning_content TEXT, 
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    cost_details JSONB DEFAULT '{}',
    usage_details JSONB DEFAULT '{}',
    reasoning_details JSONB DEFAULT '{}'
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


/* Inserts */
INSERT INTO app_config (key, value) VALUES 
    ('allow_user_providers', 'false')
ON CONFLICT (key) DO NOTHING;

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
CREATE INDEX IF NOT EXISTS idx_messages_chat ON messages(chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at);

--- Translations
INSERT INTO i18n_translations (language, key_path, value) VALUES
    -- Common
    ('en', 'common.copy_to_clipboard', 'Copied to clipboard'),
    ('de', 'common.copy_to_clipboard', 'In die Zwischenablage kopiert'),
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

    -- Common
    ('en', 'common.error', 'Error'),
    ('en', 'common.loading', 'Loading...'),
    ('en', 'common.user', 'User'),
    ('de', 'common.error', 'Fehler'),
    ('de', 'common.loading', 'Wird geladen...'),
    ('de', 'common.user', 'Benutzer'),

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
    ('en', 'settings.appearance.language', 'Language'),

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
    ('de', 'settings.appearance.language', 'Sprache'),

    -- Common
    ('en', 'common.cancel', 'Cancel'),  
    ('en', 'common.save', 'Save'),
    ('en', 'common.delete', 'Delete'),
    ('de', 'common.cancel', 'Abbrechen'),
    ('de', 'common.save', 'Speichern'),
    ('de', 'common.delete', 'Löschen'),

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
    ('en', 'chat.empty_state.desc_3', "Need help with a project? Just describe what you're working on."),
    ('en', 'chat.empty_state.desc_4', "I'm here to assist with research, explanations, or problem-solving."),
    ('en', 'chat.empty_state.desc_5', "Start a conversation and let's explore ideas together."),

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

    -- Model Selector
    ('en', 'chat.model_selector.search_models', 'Search models...'),
    ('en', 'chat.model_selector.select_model', 'Select model'),
    ('en', 'chat.model_selector.no_models', 'No models found'),
    ('en', 'chat.model_selector.tool_calling', 'Tool calling'),
    ('en', 'chat.model_selector.reasoning_capable', 'Reasoning capable'),
    ('en', 'chat.model_selector.toggle_favorite', 'Toggle favorite'),

    -- Reasoning Selector
    ('en', 'chat.reasoning_selector.auto', 'Auto'),
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

    -- Model Selector
    ('de', 'chat.model_selector.search_models', 'Modelle suchen...'),
    ('de', 'chat.model_selector.select_model', 'Modell auswählen'),
    ('de', 'chat.model_selector.no_models', 'Keine Modelle gefunden'),
    ('de', 'chat.model_selector.tool_calling', 'Tool-Aufruf'),
    ('de', 'chat.model_selector.reasoning_capable', 'Reasoning-fähig'),
    ('de', 'chat.model_selector.toggle_favorite', 'Favorit umschalten'),

    -- Reasoning Selector
    ('de', 'chat.reasoning_selector.auto', 'Auto'),
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
    ('de', 'chat.streaming_message.assistant', 'Assistent')

ON CONFLICT (language, key_path) DO NOTHING;
