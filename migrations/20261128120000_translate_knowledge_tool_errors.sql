-- Errors the knowledge and image tools report in chats.

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'chat.errors.knowledge_search_failed', 'The knowledge search failed: {reason}'),
    ('de', 'chat.errors.knowledge_search_failed', 'Die Wissenssuche ist fehlgeschlagen: {reason}'),
    ('en', 'chat.errors.knowledge_image_model_not_allowed', 'This chat contains knowledge that the configured image model may not see, so no image was generated.'),
    ('de', 'chat.errors.knowledge_image_model_not_allowed', 'Dieser Chat enthält Wissen, das das eingestellte Bildmodell nicht sehen darf, daher wurde kein Bild erzeugt.'),
    ('en', 'rag.error.database', 'The knowledge settings could not be read.'),
    ('de', 'rag.error.database', 'Die Wissenseinstellungen konnten nicht gelesen werden.')
ON CONFLICT (language, key_path) DO NOTHING;
