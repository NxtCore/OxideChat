ALTER TABLE usage_events ADD COLUMN IF NOT EXISTS api_key_id UUID REFERENCES gateway_api_keys(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_usage_events_api_key_created ON usage_events(api_key_id, created_at);

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.analytics.unknown_api_key', 'Unknown key'),
    ('de', 'settings.analytics.unknown_api_key', 'Unbekannter Schlüssel')
ON CONFLICT (language, key_path) DO NOTHING;
