UPDATE tools
SET settings_schema = jsonb_build_object(
	'type', 'object',
	'required', jsonb_build_array('image_model_id'),
	'properties', jsonb_build_object(
		'image_model_id', jsonb_build_object(
			'type', 'string',
			'title', 'Image model',
			'format', 'model-picker',
			'description', 'Select an enabled provider-backed image model'
		)
	)
)
WHERE source_kind = 'BUILTIN' AND source_config->>'builtin_id' = 'imagegen';

WITH candidate_models AS (
	SELECT DISTINCT ON (uts.id)
		uts.id AS settings_id,
		m.id AS model_id
	FROM user_tool_settings uts
	JOIN tools t ON t.id = uts.tool_id
	JOIN providers p ON (
		(LOWER(COALESCE(uts.settings->>'provider', 'openai')) = 'openai' AND p.kind = 'OPENAI')
		OR (LOWER(uts.settings->>'provider') = 'google' AND p.kind = 'GOOGLE')
	)
	JOIN models m ON m.provider_id = p.id
		AND m.model_id = COALESCE(
			uts.settings->>'model',
			CASE LOWER(COALESCE(uts.settings->>'provider', 'openai'))
				WHEN 'google' THEN 'imagen-3.0-generate-002'
				ELSE 'dall-e-3'
			END
		)
	WHERE t.source_kind = 'BUILTIN'
		AND t.source_config->>'builtin_id' = 'imagegen'
		AND NOT uts.settings ? 'image_model_id'
		AND p.is_enabled = true
		AND m.is_enabled = true
		AND EXISTS (
			SELECT 1
			FROM jsonb_array_elements_text(COALESCE(m.output_modalities, '[]'::jsonb)) AS modality(value)
			WHERE LOWER(modality.value) = 'image'
		)
	ORDER BY uts.id, p.id, m.id
)
UPDATE user_tool_settings uts
SET settings = uts.settings || jsonb_build_object('image_model_id', candidate_models.model_id::text)
FROM candidate_models
WHERE uts.id = candidate_models.settings_id;

DELETE FROM user_tool_settings uts
USING tools t
WHERE uts.tool_id = t.id
	AND t.source_kind = 'BUILTIN'
	AND t.source_config->>'builtin_id' = 'imagegen'
	AND NOT uts.settings ? 'image_model_id';

ALTER TABLE images ADD COLUMN IF NOT EXISTS caption TEXT;
ALTER TABLE tools ADD COLUMN IF NOT EXISTS system_prompt TEXT;

UPDATE tools
SET system_prompt = 'Generated images are displayed to the user automatically by the interface. In your replies to the user, never write image URLs, markdown image tags (![...](...)), or <img> tags — refer to images in words only (for example, "I''ve generated the image above"). To edit or reference a previously generated image, never invent a URL: call imagegen_edit with the image_id shown for that image in the conversation.'
WHERE source_kind = 'BUILTIN' AND source_config->>'builtin_id' = 'imagegen' AND system_prompt IS NULL;

INSERT INTO i18n_translations (language, key_path, value)
VALUES
	('en', 'settings.tools.image_model_title', 'Image model'),
	('en', 'settings.tools.image_model_description', 'Select the global image model used by this tool.'),
	('en', 'settings.tools.image_model_placeholder', 'Select image model'),
	('en', 'settings.tools.system_prompt', 'System prompt injection'),
	('en', 'settings.tools.system_prompt_placeholder', 'Extra system instructions added when this tool is active'),
	('en', 'settings.tools.system_prompt_hint', 'Prepended to the system prompt whenever this tool is enabled for a request. Use it to guide how the model uses the tool.'),
	('en', 'settings.tools.imagegen.display_name', 'Image Generation'),
	('en', 'settings.tools.imagegen.description', 'Generate and edit images using an admin-selected provider model'),
	('en', 'settings.tools.imagegen.generate.description', 'Generate an image from a text prompt'),
	('en', 'settings.tools.imagegen.generate.prompt', 'The text prompt describing the image to generate'),
	('en', 'settings.tools.imagegen.generate.size', 'Image size'),
	('en', 'settings.tools.imagegen.generate.quality', 'Image quality'),
	('en', 'settings.tools.imagegen.edit.description', 'Edit a previously generated image using a text prompt'),
	('en', 'settings.tools.imagegen.edit.image_id', 'The image ID of a previously generated image to edit. Preferred over an image URL.'),
	('en', 'settings.tools.imagegen.edit.image_url', 'URL of an external image to edit. Only use when no image ID is available.'),
	('en', 'settings.tools.imagegen.edit.prompt', 'The text prompt describing the desired edit'),
	('en', 'settings.tools.imagegen.settings.description', 'Select an enabled image model'),
	('de', 'settings.tools.image_model_title', 'Bildmodell'),
	('de', 'settings.tools.image_model_description', 'Wählen Sie das globale Bildmodell aus, das dieses Tool verwendet.'),
	('de', 'settings.tools.image_model_placeholder', 'Bildmodell auswählen'),
	('de', 'settings.tools.system_prompt', 'System-Prompt-Einschub'),
	('de', 'settings.tools.system_prompt_placeholder', 'Zusätzliche Systemanweisungen, wenn dieses Tool aktiv ist'),
	('de', 'settings.tools.system_prompt_hint', 'Wird dem System-Prompt vorangestellt, sobald dieses Tool für eine Anfrage aktiviert ist. Damit steuern Sie, wie das Modell das Tool verwendet.'),
	('de', 'settings.tools.imagegen.display_name', 'Bildgenerierung'),
	('de', 'settings.tools.imagegen.description', 'Bilder mit einem von der Administration ausgewählten Anbietermodell erstellen und bearbeiten'),
	('de', 'settings.tools.imagegen.generate.description', 'Ein Bild anhand einer Texteingabe erstellen'),
	('de', 'settings.tools.imagegen.generate.prompt', 'Die Texteingabe, die das zu erstellende Bild beschreibt'),
	('de', 'settings.tools.imagegen.generate.size', 'Bildgröße'),
	('de', 'settings.tools.imagegen.generate.quality', 'Bildqualität'),
	('de', 'settings.tools.imagegen.edit.description', 'Ein zuvor erstelltes Bild anhand einer Texteingabe bearbeiten'),
	('de', 'settings.tools.imagegen.edit.image_id', 'Die Bild-ID eines zuvor erstellten Bildes. Diese wird gegenüber einer Bild-URL bevorzugt.'),
	('de', 'settings.tools.imagegen.edit.image_url', 'URL eines externen Bildes. Nur verwenden, wenn keine Bild-ID verfügbar ist.'),
	('de', 'settings.tools.imagegen.edit.prompt', 'Die Texteingabe, die die gewünschte Bearbeitung beschreibt'),
	('de', 'settings.tools.imagegen.settings.description', 'Ein aktiviertes Bildmodell auswählen')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;

UPDATE tool_functions tf
SET input_schema = jsonb_build_object(
	'type', 'object',
	'required', jsonb_build_array('prompt'),
	'properties', jsonb_build_object(
		'image_id', jsonb_build_object(
			'type', 'string',
			'description', 'The image_id of a previously generated image to edit (shown in the conversation). Preferred over image_url.'
		),
		'image_url', jsonb_build_object(
			'type', 'string',
			'description', 'URL of an external image to edit. Only use when no image_id is available.'
		),
		'prompt', jsonb_build_object(
			'type', 'string',
			'description', 'The text prompt describing the desired edit'
		)
	)
)
FROM tools t
WHERE tf.tool_id = t.id
	AND t.source_kind = 'BUILTIN'
	AND t.source_config->>'builtin_id' = 'imagegen'
	AND tf.name = 'edit';

UPDATE tool_functions tf
SET input_schema = jsonb_set(
	jsonb_set(tf.input_schema, '{properties,quality,enum}', '["auto", "low", "medium", "high"]'::jsonb, true),
	'{properties,quality,default}', '"auto"'::jsonb, true
)
FROM tools t
WHERE tf.tool_id = t.id
	AND t.source_kind = 'BUILTIN'
	AND t.source_config->>'builtin_id' = 'imagegen'
	AND tf.name = 'generate'
	AND tf.input_schema -> 'properties' ? 'quality';
