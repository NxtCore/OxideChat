INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.model_search.placeholder', 'Search models...'),
    ('en', 'developer.model_search.empty', 'No models match your search'),
    ('de', 'developer.model_search.placeholder', 'Modelle suchen...'),
    ('de', 'developer.model_search.empty', 'Keine Modelle entsprechen deiner Suche')
ON CONFLICT (language, key_path) DO NOTHING;
