INSERT INTO i18n_translations (language, key_path, value)
VALUES
    ('en', 'settings.analytics.top_api_keys', 'Top API Keys'),
    ('en', 'settings.analytics.coming_soon', 'Coming soon'),
    ('en', 'settings.analytics.api_keys_soon', 'API key tracking will be available in a future release'),
    ('en', 'settings.analytics.total_tokens_short', 'Tokens'),
    ('en', 'settings.analytics.custom_range', 'Custom range'),
    ('en', 'settings.analytics.preset_last_7d', 'Last 7 days'),
    ('en', 'settings.analytics.preset_last_30d', 'Last 30 days'),
    ('en', 'settings.analytics.preset_last_90d', 'Last 90 days'),
    ('en', 'settings.analytics.preset_this_month', 'This month'),
    ('en', 'settings.analytics.preset_last_month', 'Last month'),
    ('en', 'settings.analytics.preset_this_year', 'This year'),

    ('de', 'settings.analytics.top_api_keys', 'Top API-Schlüssel'),
    ('de', 'settings.analytics.coming_soon', 'Demnächst'),
    ('de', 'settings.analytics.api_keys_soon', 'API-Schlüssel-Tracking wird in einem zukünftigen Release verfügbar sein'),
    ('de', 'settings.analytics.total_tokens_short', 'Tokens'),
    ('de', 'settings.analytics.custom_range', 'Benutzerdefinierter Zeitraum'),
    ('de', 'settings.analytics.preset_last_7d', 'Letzte 7 Tage'),
    ('de', 'settings.analytics.preset_last_30d', 'Letzte 30 Tage'),
    ('de', 'settings.analytics.preset_last_90d', 'Letzte 90 Tage'),
    ('de', 'settings.analytics.preset_this_month', 'Diesen Monat'),
    ('de', 'settings.analytics.preset_last_month', 'Letzten Monat'),
    ('de', 'settings.analytics.preset_this_year', 'Dieses Jahr')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;
