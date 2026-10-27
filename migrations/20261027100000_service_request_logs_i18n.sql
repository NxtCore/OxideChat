INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.logs.scope', 'Show'),
    ('en', 'developer.logs.scope_mine', 'My requests'),
    ('en', 'developer.logs.scope_service', 'Service keys of {team}'),
    ('de', 'developer.logs.scope', 'Anzeigen'),
    ('de', 'developer.logs.scope_mine', 'Meine Anfragen'),
    ('de', 'developer.logs.scope_service', 'Service-Schlüssel von {team}')
ON CONFLICT (language, key_path) DO NOTHING;
