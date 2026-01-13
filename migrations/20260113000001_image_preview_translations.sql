-- Add translations for image preview component
INSERT INTO i18n_translations (language, key_path, value) VALUES
    -- English
    ('en', 'chat.image_preview.download', 'Download'),
    ('en', 'chat.image_preview.copy', 'Copy URL'),
    ('en', 'chat.image_preview.copied', 'Copied!'),
    ('en', 'chat.tool_execution.generated_image', 'Generated Image'),

    -- German
    ('de', 'chat.image_preview.download', 'Herunterladen'),
    ('de', 'chat.image_preview.copy', 'URL kopieren'),
    ('de', 'chat.image_preview.copied', 'Kopiert!'),
    ('de', 'chat.tool_execution.generated_image', 'Generiertes Bild');
