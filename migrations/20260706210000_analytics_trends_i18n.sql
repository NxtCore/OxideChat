INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.spend', 'Spend'),
    ('en', 'settings.analytics.section_models', 'Models'),
    ('en', 'settings.analytics.section_api_keys', 'API Keys'),
    ('en', 'settings.analytics.section_user', 'User'),
    ('en', 'settings.analytics.trend', 'Trend'),

    ('de', 'settings.analytics.spend', 'Ausgaben'),
    ('de', 'settings.analytics.section_models', 'Modelle'),
    ('de', 'settings.analytics.section_api_keys', 'API-Schlüssel'),
    ('de', 'settings.analytics.section_user', 'Benutzer'),
    ('de', 'settings.analytics.trend', 'Trend')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;
