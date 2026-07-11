INSERT INTO i18n_translations (language, key_path, value)
VALUES
	('en', 'settings.tools.image_model_title', 'Image model'),
	('en', 'settings.tools.image_model_description', 'Select the global image model used by this tool.'),
	('en', 'settings.tools.image_model_placeholder', 'Select image model'),
	('de', 'settings.tools.image_model_title', 'Bildmodell'),
	('de', 'settings.tools.image_model_description', 'Wählen Sie das globale Bildmodell aus, das dieses Tool verwendet.'),
	('de', 'settings.tools.image_model_placeholder', 'Bildmodell auswählen')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;
