-- Permissions and role-permission mapping
-- Enables extensible permission system linked to roles

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

CREATE INDEX IF NOT EXISTS idx_role_permissions_role_id ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id ON role_permissions(permission_id);

-- Seed default permissions (including wildcards)
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
    ('admin.config.edit', 'Edit application config')
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

-- Settings page translations
INSERT INTO i18n_translations (language, key_path, value) VALUES
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
    ('de', 'settings.appearance.language', 'Sprache')
ON CONFLICT (language, key_path) DO NOTHING;
