macro_rules! preset_columns {
	() => {
		"id, scope, owner_id, slug, name, description, icon, base_model_id, system_prompt, sampling, reasoning, tool_ids, \
		 tools_locked, starters, prompt_visible, is_enabled, updated_at"
	};
}

pub(super) use preset_columns;
