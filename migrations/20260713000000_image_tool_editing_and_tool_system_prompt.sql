-- Image captions (text stand-in for non-vision models) and per-tool system prompt injection.
ALTER TABLE images ADD COLUMN IF NOT EXISTS caption TEXT;
ALTER TABLE tools ADD COLUMN IF NOT EXISTS system_prompt TEXT;

-- Default system-prompt injection for the built-in image tool: never emit image URLs to the
-- user, and edit previous images by their image_id instead of inventing a URL.
UPDATE tools
SET system_prompt = 'Generated images are displayed to the user automatically by the interface. In your replies to the user, never write image URLs, markdown image tags (![...](...)), or <img> tags — refer to images in words only (for example, "I''ve generated the image above"). To edit or reference a previously generated image, never invent a URL: call imagegen_edit with the image_id shown for that image in the conversation.'
WHERE source_kind = 'BUILTIN' AND source_config->>'builtin_id' = 'imagegen' AND system_prompt IS NULL;

-- Translations for the admin tool editor's system-prompt field.
INSERT INTO i18n_translations (language, key_path, value)
VALUES
	('en', 'settings.tools.system_prompt', 'System prompt injection'),
	('en', 'settings.tools.system_prompt_placeholder', 'Extra system instructions added when this tool is active'),
	('en', 'settings.tools.system_prompt_hint', 'Prepended to the system prompt whenever this tool is enabled for a request. Use it to guide how the model uses the tool.'),
	('de', 'settings.tools.system_prompt', 'System-Prompt-Einschub'),
	('de', 'settings.tools.system_prompt_placeholder', 'Zusätzliche Systemanweisungen, wenn dieses Tool aktiv ist'),
	('de', 'settings.tools.system_prompt_hint', 'Wird dem System-Prompt vorangestellt, sobald dieses Tool für eine Anfrage aktiviert ist. Damit steuern Sie, wie das Modell das Tool verwendet.')
ON CONFLICT (language, key_path) DO UPDATE SET value = EXCLUDED.value;

-- imagegen `edit` now takes an image_id (preferred) so the model never fabricates a URL.
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

-- imagegen `generate` constrains `quality` to the provider-accepted values.
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
