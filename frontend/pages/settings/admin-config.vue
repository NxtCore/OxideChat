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

			<SettingsRow :label="store.getTranslation('settings.appearance.language')" :hint="store.getTranslation('settings.admin_config.language_hint')" stacked>
				<ShadSelect v-model="form.language" :disabled="!canEdit || settingsForm.saving">
					<ShadSelectTrigger data-testid="config-language">
						<ShadSelectValue />
					</ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem value="en">{{ store.getTranslation('settings.appearance.language_en') }}</ShadSelectItem>
						<ShadSelectItem value="de">{{ store.getTranslation('settings.appearance.language_de') }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('settings.admin_config.default_theme')" :hint="store.getTranslation('settings.admin_config.default_theme_hint')" stacked>
				<ShadSelect v-model="selectedTheme" :disabled="!canEdit || settingsForm.saving">
					<ShadSelectTrigger data-testid="config-default-theme">
						<ShadSelectValue />
					</ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem :value="NO_THEME">{{ store.getTranslation('settings.admin_config.default_theme_none') }}</ShadSelectItem>
						<ShadSelectItem v-if="selectedTheme === CUSTOM_THEME" :value="CUSTOM_THEME">{{ store.getTranslation('settings.admin_config.default_theme_custom') }}</ShadSelectItem>
						<ShadSelectItem v-for="theme in availableThemes" :key="theme.url" :value="theme.url">{{ theme.name }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
			</SettingsRow>
		</div>

		<SettingsOAuthProvidersCard />
	</SettingsPage>
</template>

<script setup lang="ts">
import {ADMIN_CONFIG_EDIT} from '~/types/generated/permissions';
import {computed, onMounted, reactive} from 'vue';
import {useMainStore} from '@/stores';
import {useThemeStore} from '~/stores/theme';
import {hasThemeVars, isSameTheme} from '~/lib/theme-utils';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';

const NO_THEME = 'none';
const CUSTOM_THEME = 'custom';

const store = useMainStore();
const themeStore = useThemeStore();
const canEdit = computed(() => store.hasPermission(ADMIN_CONFIG_EDIT));

const form = reactive({
	enable_provider_selector: store.base?.enable_provider_selector ?? false,
	allow_server_stdio_mcp: store.base?.allow_server_stdio_mcp ?? false,
	default_model_id: store.base?.default_model_id ?? null,
	language: store.base?.language ?? 'en',
	default_theme: store.base?.default_theme ?? {theme: {}, light: {}, dark: {}},
});

const availableThemes = computed(() => themeStore.fetchedThemes.filter(theme => !theme.error));

const selectedTheme = computed({
	get() {
		if (!hasThemeVars(form.default_theme)) return NO_THEME;
		return availableThemes.value.find(theme => isSameTheme(theme.preset.cssVars, form.default_theme))?.url ?? CUSTOM_THEME;
	},
	set(value: string) {
		if (value === NO_THEME) form.default_theme = {theme: {}, light: {}, dark: {}};
		const theme = availableThemes.value.find(item => item.url === value);
		if (theme) form.default_theme = theme.preset.cssVars;
	},
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
		themeStore.setDefaultTheme(form.default_theme);
	},
});

onMounted(() => themeStore.fetchAllThemes());
</script>
