INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.admin_users.view_analytics', 'View analytics'),
    ('de', 'settings.admin_users.view_analytics', 'Analysen anzeigen')
ON CONFLICT (language, key_path) DO NOTHING;
