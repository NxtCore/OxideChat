<template>
	<SettingsPage :title="store.getTranslation('settings.tabs.admin_config')" :description="store.getTranslation('settings.admin_config.description')" layout="form" :form="settingsForm">
		<div class="rounded-lg border border-border bg-card p-6">
			<SettingsRow :label="store.getTranslation('settings.admin_config.provider_selector.label')" :hint="store.getTranslation('settings.admin_config.provider_selector.hint')">
				<ShadSwitch v-model="form.enable_provider_selector" :disabled="!canEdit || settingsForm.saving" data-testid="config-provider-selector" />
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('admin.tools.mcp.allow_stdio')" :hint="store.getTranslation('admin.tools.mcp.allow_stdio_hint')" warning>
				<ShadSwitch v-model="form.allow_server_stdio_mcp" :disabled="!canEdit || settingsForm.saving" data-testid="config-allow-stdio" />
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('settings.admin.default_model')" :hint="store.getTranslation('settings.admin.default_model_hint')" stacked>
				<DefaultModelPicker
					v-model="form.default_model_id"
					:disabled="!canEdit || settingsForm.saving"
					:endpoint="adminModelsPath"
					:selected-model-endpoint="adminModelsPath"
					value-mode="uuid"
					:placeholder="store.getTranslation('settings.teams.use_global_default')"
				/>
			</SettingsRow>
		</div>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ADMIN_CONFIG_EDIT} from '~/types/generated/permissions';
import {computed, reactive} from 'vue';
import {useMainStore} from '@/stores';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';

const store = useMainStore();
const canEdit = computed(() => store.hasPermission(ADMIN_CONFIG_EDIT));

const form = reactive({
	enable_provider_selector: store.base?.enable_provider_selector ?? false,
	allow_server_stdio_mcp: store.base?.allow_server_stdio_mcp ?? false,
	default_model_id: store.base?.default_model_id ?? null,
});

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	canEdit: () => canEdit.value,
	savedKey: 'settings.admin_config.save_success',
	errorKey: 'settings.admin_config.save_error',
	save: async () => {
		await patchAdminConfig(form);
		if (store.base) Object.assign(store.base, form);
	},
});
</script>
