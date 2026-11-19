INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.endpoints.anthropic', 'Anthropic Messages'),
    ('en', 'developer.endpoints.anthropic_hint', 'For Claude Code, set ANTHROPIC_BASE_URL to this URL and ANTHROPIC_API_KEY to your API key.'),
    ('de', 'developer.endpoints.anthropic', 'Anthropic Messages'),
    ('de', 'developer.endpoints.anthropic_hint', 'Für Claude Code setze ANTHROPIC_BASE_URL auf diese URL und ANTHROPIC_API_KEY auf deinen API-Schlüssel.')
ON CONFLICT (language, key_path) DO NOTHING;
