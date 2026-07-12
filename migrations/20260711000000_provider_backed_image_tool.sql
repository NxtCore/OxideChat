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

UPDATE user_tool_settings uts
SET settings = uts.settings || jsonb_build_object('image_model_id', m.id::text)
FROM tools t
JOIN models m ON true
JOIN providers p ON p.id = m.provider_id
WHERE uts.tool_id = t.id
	AND t.source_kind = 'BUILTIN'
	AND t.source_config->>'builtin_id' = 'imagegen'
	AND NOT uts.settings ? 'image_model_id'
	AND m.model_id = COALESCE(
		uts.settings->>'model',
		CASE LOWER(COALESCE(uts.settings->>'provider', 'openai'))
			WHEN 'google' THEN 'imagen-3.0-generate-002'
			ELSE 'dall-e-3'
		END
	)
	AND (
		(LOWER(COALESCE(uts.settings->>'provider', 'openai')) = 'openai' AND p.kind = 'OPENAI')
		OR (LOWER(uts.settings->>'provider') = 'google' AND p.kind = 'GOOGLE')
	);
