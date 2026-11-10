<template>
	<ShadDropdownMenu v-if="preset || canCreate">
		<ShadDropdownMenuTrigger as-child>
			<ShadButton
				variant="ghost"
				data-testid="preset-menu"
				:class="cn('h-8 w-8 p-0', preset ? 'text-primary' : 'text-muted-foreground')"
				:aria-label="store.getTranslation('presets.menu.title')"
			>
				<Wand2 class="h-4 w-4" />
			</ShadButton>
		</ShadDropdownMenuTrigger>
		<ShadDropdownMenuContent align="start" class="w-60 border-border bg-popover">
			<template v-if="preset">
				<ShadDropdownMenuLabel class="text-popover-foreground">
					<p class="truncate">{{ preset.name }}</p>
					<p class="truncate text-xs font-normal text-muted-foreground">{{ preset.key }}</p>
				</ShadDropdownMenuLabel>
				<ShadDropdownMenuSeparator class="bg-border" />
				<ShadDropdownMenuItem v-if="preset.system_prompt" data-testid="preset-menu-instructions" class="text-popover-foreground focus:bg-accent" @click="instructionsOpen = true">
					<ScrollText class="mr-2 h-4 w-4" />
					{{ store.getTranslation('presets.menu.view_instructions') }}
				</ShadDropdownMenuItem>
				<ShadDropdownMenuItem v-if="preset.can_edit" data-testid="preset-menu-edit" class="text-popover-foreground focus:bg-accent" @click="openSettings(`/settings/presets/${preset.id}`)">
					<Pencil class="mr-2 h-4 w-4" />
					{{ store.getTranslation('presets.menu.edit') }}
				</ShadDropdownMenuItem>
			</template>
			<ShadDropdownMenuItem v-if="canCreate" data-testid="preset-menu-save" class="text-popover-foreground focus:bg-accent" @click="saveAsPreset">
				<BookmarkPlus class="mr-2 h-4 w-4" />
				{{ store.getTranslation('presets.menu.save_as') }}
			</ShadDropdownMenuItem>
		</ShadDropdownMenuContent>
	</ShadDropdownMenu>

	<ShadDialog v-model:open="instructionsOpen">
		<ShadDialogContent class="max-w-2xl">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('presets.menu.instructions_title', {preset: preset?.name ?? ''}) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('presets.menu.instructions_description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<pre class="max-h-[60vh] overflow-y-auto whitespace-pre-wrap rounded-lg bg-muted p-3 font-mono text-xs text-foreground" data-testid="preset-instructions">{{ preset?.system_prompt }}</pre>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {BookmarkPlus, Pencil, ScrollText, Wand2} from 'lucide-vue-next';
import {cn} from '~/lib/utils';
import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {usePresetStore} from '~/stores/presetStore';
import {SETTINGS_PRESETS_EDIT} from '~/types/generated/permissions';
import type {PresetSampling} from '~/types/presets';

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();
const presetStore = usePresetStore();
const route = useRoute();
const router = useRouter();

const instructionsOpen = ref(false);
const preset = computed(() => modelStore.selectedModel?.preset ?? null);
const canCreate = computed(() => store.hasPermission(SETTINGS_PRESETS_EDIT));

function openSettings(path: string) {
	store.settingsReturnPath = route.path.startsWith('/chats/') ? route.fullPath : '/';
	router.push(path);
}

function saveAsPreset() {
	const model = modelStore.selectedModel;
	const chatSampling = chatStore.activeChat?.sampling ?? chatStore.draftSampling;
	const sampling: PresetSampling = {...(preset.value?.sampling ?? {})};
	for (const key of ['temperature', 'top_p', 'max_tokens', 'frequency_penalty', 'presence_penalty', 'seed'] as const) {
		const value = chatSampling[key];
		if (typeof value === 'number') (sampling as Record<string, number>)[key] = value;
	}
	presetStore.draft = {
		slug: '',
		name: preset.value ? store.getTranslation('presets.copy_name', {name: preset.value.name}) : '',
		description: preset.value?.description ?? null,
		icon: preset.value?.icon ?? null,
		base_model_id: model ? baseModelId(model) : '',
		base_model: preset.value?.base_model ?? model ?? null,
		system_prompt: preset.value?.system_prompt ?? null,
		sampling,
		reasoning: {effort: modelStore.reasoningEffort ?? undefined, budget_tokens: modelStore.reasoningBudget ?? undefined},
		tool_ids: [...modelStore.enabledTools],
		tools_locked: preset.value?.tools_locked ?? false,
		starters: [...(preset.value?.starters ?? [])],
		prompt_visible: true,
		is_enabled: true,
	};
	openSettings('/settings/presets/new');
}
</script>
