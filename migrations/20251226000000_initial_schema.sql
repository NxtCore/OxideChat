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


-- Default roles
INSERT INTO roles (name) VALUES ('admin'), ('user') ON CONFLICT DO NOTHING;

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


--- Translations
INSERT INTO i18n_translations (language, key_path, value) VALUES
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
        -- Sidebar
    ('en', 'sidebar.chats', 'Chats'),
    ('en', 'sidebar.new_chat', 'New Chat'),
    ('en', 'sidebar.logout', 'Log out'),
    ('en', 'sidebar.ai_chat_app', 'AI Chat Application'),
    ('de', 'sidebar.chats', 'Chats'),
    ('de', 'sidebar.new_chat', 'Neuer Chat'),
    ('de', 'sidebar.logout', 'Abmelden'),
    ('de', 'sidebar.ai_chat_app', 'KI-Chat-Anwendung'),

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
    ('de', 'auth.password.strength_good', 'Gut')
ON CONFLICT (language, key_path) DO NOTHING;
