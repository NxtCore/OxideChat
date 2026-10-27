INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.analytics.service_accounts', 'Service accounts'),
    ('de', 'settings.analytics.service_accounts', 'Servicekonten')
ON CONFLICT (language, key_path) DO NOTHING;
