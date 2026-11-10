macro_rules! preset_columns {
	() => {
		"id, scope, owner_id, team_id, (SELECT t.name FROM teams t WHERE t.id = model_presets.team_id) AS team_name, slug, name, description, icon, base_model_id, system_prompt, sampling, reasoning, tool_ids, \
		 tools_locked, starters, prompt_visible, is_enabled, updated_at"
	};
}

pub(super) use preset_columns;
