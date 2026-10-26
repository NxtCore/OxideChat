INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.keys.admin_disabled', 'Disabled by administrator'),
    ('de', 'developer.keys.admin_disabled', 'Vom Administrator deaktiviert')
ON CONFLICT (language, key_path) DO NOTHING;
