INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.logs.category_internal', 'Internal error'),
    ('en', 'developer.logs.model_placeholder', 'Any model'),
    ('en', 'developer.logs.provider_placeholder', 'Any provider'),
    ('de', 'developer.logs.category_internal', 'Interner Fehler'),
    ('de', 'developer.logs.model_placeholder', 'Beliebiges Modell'),
    ('de', 'developer.logs.provider_placeholder', 'Beliebiger Anbieter')
ON CONFLICT (language, key_path) DO NOTHING;
