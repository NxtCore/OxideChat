INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'chat.provider_selector.col_uptime', 'Uptime'),
    ('en', 'chat.provider_selector.price_io_label', 'Input / Output'),
    ('en', 'chat.provider_selector.latency_estimated', 'Estimated'),
    ('en', 'chat.provider_selector.context_tokens', 'Tokens'),
    ('en', 'chat.provider_selector.status_healthy', 'Healthy'),
    ('en', 'chat.provider_selector.status_degraded', 'Degraded'),
    ('en', 'chat.provider_selector.status_all_operational', 'All systems operational'),
    ('en', 'chat.provider_selector.status_some_issues', 'Some issues detected'),
    ('de', 'chat.provider_selector.col_uptime', 'Verfügbarkeit'),
    ('de', 'chat.provider_selector.price_io_label', 'Ein / Aus'),
    ('de', 'chat.provider_selector.latency_estimated', 'Geschätzt'),
    ('de', 'chat.provider_selector.context_tokens', 'Token'),
    ('de', 'chat.provider_selector.status_healthy', 'Gesund'),
    ('de', 'chat.provider_selector.status_degraded', 'Beeinträchtigt'),
    ('de', 'chat.provider_selector.status_all_operational', 'Alle Systeme betriebsbereit'),
    ('de', 'chat.provider_selector.status_some_issues', 'Einige Probleme erkannt')
ON CONFLICT (language, key_path) DO NOTHING;
