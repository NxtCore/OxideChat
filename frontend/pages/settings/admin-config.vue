<template>
	<SettingsPage :title="store.getTranslation('settings.tabs.admin_config')" :description="store.getTranslation('settings.admin_config.description')" layout="form" :form="settingsForm">
		<div class="rounded-lg border border-border bg-card p-6">
			<SettingsRow :label="store.getTranslation('settings.admin_config.instance_name')" :hint="store.getTranslation('settings.admin_config.instance_name_hint')" stacked>
				<ShadInput v-model="form.instance.name" :maxlength="64" placeholder="OxideChat" :disabled="!canEdit || settingsForm.saving" data-testid="config-instance-name" />
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('settings.admin_config.instance_icon')" :hint="store.getTranslation('settings.admin_config.instance_icon_hint')" stacked>
				<ImagePicker
					v-model="form.instance.icon_id"
					value-mode="id"
					:allow-link="false"
					:disabled="!canEdit || settingsForm.saving"
					:title="store.getTranslation('settings.admin_config.instance_icon')"
					test-id="config-instance-icon"
				>
					<template #fallback><img src="/light_transparent.svg" alt="" class="h-full w-full object-contain p-2" /></template>
				</ImagePicker>
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('settings.admin_config.provider_selector.label')" :hint="store.getTranslation('settings.admin_config.provider_selector.hint')">
				<ShadSwitch v-model="form.provider_routing.selector_enabled" :disabled="!canEdit || settingsForm.saving" data-testid="config-provider-selector" />
			</SettingsRow>

			<SettingsRow :label="store.getTranslation('settings.admin_config.allowed_providers.label')" :hint="store.getTranslation('settings.admin_config.allowed_providers.hint')" stacked>
				<ProviderAllowlistEditor v-model="form.provider_routing.allowed_providers" :disabled="!canEdit || settingsForm.saving" :can-sync="canEdit" test-id="config-allowed-providers" />
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

			<SettingsRow :label="store.getTranslation('settings.admin_config.title_generation')" :hint="store.getTranslation('settings.admin_config.title_generation_hint')">
				<ShadSwitch v-model="form.title_generation.enabled" :disabled="!canEdit || settingsForm.saving" data-testid="config-title-generation" />
			</SettingsRow>

			<SettingsRow
				v-if="form.title_generation.enabled"
				:label="store.getTranslation('settings.admin_config.title_model')"
				:hint="store.getTranslation('settings.admin_config.title_model_hint')"
				stacked
			>
				<DefaultModelPicker
					v-model="form.title_generation.model_id"
					:disabled="!canEdit || settingsForm.saving"
					:endpoint="adminModelsPath"
					:selected-model-endpoint="adminModelsPath"
					value-mode="uuid"
					:placeholder="store.getTranslation('settings.admin_config.title_model_chat')"
					:params="{output_only: 'text'}"
					@selected="titleModel = $event"
				/>
				<div v-if="titleModelReasons" class="mt-2 flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 p-3 text-xs text-foreground" role="alert" data-testid="config-title-model-reasoning-warning">
					<AlertTriangle class="mt-0.5 h-3.5 w-3.5 shrink-0 text-destructive" />
					<span>{{ store.getTranslation('settings.admin_config.title_model_reasoning_warning') }}</span>
				</div>
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
import {computed, onMounted, reactive, ref} from 'vue';
import {useMainStore} from '@/stores';
import {useThemeStore} from '~/stores/theme';
import {hasThemeVars, isSameTheme} from '~/lib/theme-utils';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import ProviderAllowlistEditor from '~/components/settings/ProviderAllowlistEditor.vue';
import {AlertTriangle} from 'lucide-vue-next';
import type {ModelList} from '~/types/chat';

const NO_THEME = 'none';
const CUSTOM_THEME = 'custom';

const store = useMainStore();
const themeStore = useThemeStore();
const canEdit = computed(() => store.hasPermission(ADMIN_CONFIG_EDIT));

const form = reactive({
	provider_routing: {
		selector_enabled: store.base?.provider_routing.selector_enabled ?? false,
		allowed_providers: copyAllowlists(store.base?.provider_routing.allowed_providers),
	},
	default_model_id: store.base?.default_model_id ?? null,
	language: store.base?.default_language ?? 'en',
	default_theme: store.base?.default_theme ?? {theme: {}, light: {}, dark: {}},
	instance: {
		name: store.base?.instance.name ?? '',
		icon_id: store.base?.instance.icon_id ?? null,
	},
	title_generation: {
		enabled: store.base?.title_generation.enabled ?? false,
		model_id: store.base?.title_generation.model_id ?? null,
	},
});

const titleModel = ref<ModelList | null>(null);
const titleModelReasons = computed(() => !!form.title_generation.model_id && !!titleModel.value?.capabilities.some(capability => capability.startsWith('REASONING')));

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
	validate: () => emptyAllowlistError(form.provider_routing.allowed_providers),
	savedKey: 'settings.admin_config.save_success',
	errorKey: 'settings.admin_config.save_error',
	save: async () => {
		const selectorChanged = store.base?.provider_routing.selector_enabled !== form.provider_routing.selector_enabled;
		await patchAdminConfig(form);
		if (selectorChanged) store.refreshChatProviderSelector();
		if (store.base) {
			const {language, ...rest} = form;
			Object.assign(store.base, JSON.parse(JSON.stringify(rest)), {default_language: language});
			store.applyLanguage();
		}
		themeStore.setDefaultTheme(form.default_theme);
	},
});

onMounted(() => themeStore.fetchAllThemes());
</script>
