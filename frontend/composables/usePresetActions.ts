import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {usePresetStore} from '~/stores/presetStore';
import {SETTINGS_PRESETS_EDIT} from '~/types/generated/permissions';
import type {PresetSampling} from '~/types/presets';

const SAMPLING_KEYS = ['temperature', 'top_p', 'max_tokens', 'frequency_penalty', 'presence_penalty', 'seed'] as const;

export function usePresetActions() {
	const store = useMainStore();
	const chatStore = useChatStore();
	const modelStore = useModelStore();
	const presetStore = usePresetStore();
	const route = useRoute();
	const router = useRouter();

	const personal = store.hasPermission(SETTINGS_PRESETS_EDIT);
	if (!personal && store.user) presetStore.ensureScopes();
	const canCreate = computed(() => personal || !!presetStore.scopes?.teams.length || !!presetStore.scopes?.installation);

	function openSettings(path: string) {
		store.settingsReturnPath = route.path.startsWith('/chats/') ? route.fullPath : '/';
		router.push(path);
	}

	function saveAsPreset() {
		const model = modelStore.selectedModel;
		const preset = model?.preset ?? null;
		const chatSampling = chatStore.activeChat?.sampling ?? chatStore.draftSampling;
		const usableLevels = preset?.levels.filter(level => !level.unavailable) ?? [];
		const levels = usableLevels.length >= MIN_PRESET_LEVELS ? usableLevels : [];
		const sampling: PresetSampling = {...(preset?.sampling ?? {})};
		for (const key of SAMPLING_KEYS) {
			const value = chatSampling[key];
			if (typeof value === 'number') (sampling as Record<string, number>)[key] = value;
		}
		presetStore.draft = {
			slug: '',
			name: preset ? store.getTranslation('presets.copy_name', {name: preset.name}) : '',
			description: preset?.description ?? null,
			icon: preset?.icon ?? null,
			base_model_id: model ? baseModelId(model) : '',
			base_model: model?.level?.model ?? preset?.base_model ?? model ?? null,
			system_prompt: preset?.system_prompt ?? null,
			sampling,
			reasoning: levels.length > 0 ? {...preset!.reasoning} : {effort: modelStore.reasoningEffort ?? undefined, budget_tokens: modelStore.reasoningBudget ?? undefined},
			levels: levels.map(levelToRequest),
			default_level: levels.find(level => level.key === model?.level?.key)?.key ?? levels.find(level => level.key === preset?.default_level)?.key ?? null,
			level_models: levels.map(level => level.model),
			tool_ids: [...modelStore.enabledTools],
			tools_locked: preset?.tools_locked ?? false,
			starters: [...(preset?.starters ?? [])],
			prompt_visible: true,
			is_enabled: true,
		};
		openSettings('/settings/presets/new');
	}

	return {canCreate, openSettings, saveAsPreset};
}
