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
SET settings = CASE
	WHEN settings ? 'image_model_id' THEN jsonb_build_object('image_model_id', settings->'image_model_id')
	ELSE '{}'::jsonb
END
FROM tools t
WHERE uts.tool_id = t.id
	AND t.source_kind = 'BUILTIN'
	AND t.source_config->>'builtin_id' = 'imagegen';
