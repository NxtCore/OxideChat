<template>
	<Popover v-model:open="isOpen">
		<PopoverTrigger as-child>
			<button
				data-testid="model-selector"
				:title="fullName"
				:aria-label="fullName"
				:class="cn('flex min-w-0 items-center gap-2 px-2 py-1 rounded-md hover:bg-accent transition-colors', props.class)"
			>
				<img
					v-if="modelStore.selectedModel?.icon"
					:src="modelStore.selectedModel?.icon"
					class="h-4 w-4 mt-0.5 shrink-0 flex items-center justify-center rounded-md bg-muted overflow-hidden"
				/>
				<Wand2 v-else-if="modelStore.selectedModel?.preset" class="h-4 w-4 shrink-0 text-primary" />
				<div
					v-else-if="iconStore.getProviderIcon(modelStore.selectedModel?.provider.name, modelStore.selectedModel?.model_id)?.type === 'svg'"
					class="h-4 w-4 shrink-0 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
					v-html="providerIconHtml(modelStore.selectedModel?.provider.name, modelStore.selectedModel?.model_id)"
				/>
				<div
					v-else-if="iconStore.getProviderIcon(modelStore.selectedModel?.provider.name, modelStore.selectedModel?.model_id)?.type === 'png'"
					class="h-4 w-4 shrink-0 flex items-center justify-center"
				>
					<img :src="iconStore.getProviderIcon(modelStore.selectedModel?.provider.name, modelStore.selectedModel?.model_id)?.icon" alt="Provider icon" />
				</div>
				<span class="min-w-0 truncate text-xs font-medium" data-testid="model-selector-label">{{ shortName }}</span>
				<ChevronDown class="h-3 w-3 shrink-0 text-muted-foreground" />
			</button>
		</PopoverTrigger>
		<PopoverContent class="w-130 max-w-[calc(100vw-1rem)] p-0 overflow-hidden" align="start" :side-offset="8">
			<div class="flex h-100">
				<div class="flex-1 flex flex-col overflow-hidden">
					<div class="p-2 border-b border-border flex items-center gap-2">
						<div class="relative flex-1">
							<Search class="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
							<ShadInput
								:model-value="pickerSearch"
								type="text"
								:placeholder="store.getTranslation('chat.model_selector.search_models')"
								class="pl-8 h-8 text-sm"
								@update:model-value="pickerOnSearch($event)"
							/>
						</div>
						<button class="p-1.5 rounded-md hover:bg-accent text-muted-foreground">
							<Filter class="h-4 w-4" />
						</button>
					</div>

					<div ref="scrollContainer" class="flex-1 overflow-y-auto">
						<div v-if="pickerLoading" class="flex items-center justify-center py-8 text-muted-foreground">
							<Loader2 class="h-5 w-5 animate-spin" />
						</div>
						<template v-else>
							<template v-if="visiblePresets.length > 0">
								<p v-if="showSectionHeaders" class="px-3 pt-2 pb-1 text-xs font-medium text-muted-foreground">
									{{ store.getTranslation('presets.picker.section') }}
								</p>
								<div
									v-for="preset in visiblePresets"
									:key="preset.id"
									data-testid="picker-preset"
									:data-preset-key="preset.key"
									class="px-3 py-2 transition-colors border-b border-border/50"
									:class="isPresetBlocked(preset) ? 'opacity-50 cursor-not-allowed' : 'hover:bg-accent/50 cursor-pointer'"
									@click="selectPreset(preset)"
									@pointerenter="activeRow = preset.key"
									@focusin="activeRow = preset.key"
								>
									<div class="flex items-start gap-3">
										<img v-if="preset.icon" :src="preset.icon" class="h-5 w-5 mt-0.5 rounded-md bg-muted object-cover" alt="" />
										<Wand2 v-else class="h-5 w-5 mt-0.5 text-primary" />
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="font-medium text-sm">{{ preset.name }}</span>
												<span class="px-1.5 py-0.5 text-xs rounded bg-primary/10 text-primary">
													{{ presetScopeLabel(preset, store.getTranslation) }}
												</span>
											</div>
											<p class="text-xs text-muted-foreground mt-0.5 truncate">
												{{ preset.base_model ? store.getTranslation('presets.picker.on_model', {model: preset.base_model.display_name}) : preset.key }}
											</p>
											<p v-if="presetProblem(preset)" class="text-xs text-destructive mt-1">
												{{ presetProblem(preset) }}
											</p>
										</div>
										<div class="flex items-center gap-1 ml-2">
											<RowTooltip v-if="preset.system_prompt" :active="activeRow === preset.key" :text="store.getTranslation('presets.menu.view_instructions')">
												<button class="p-1 rounded hover:bg-accent text-muted-foreground" data-testid="picker-preset-instructions" @click.stop="showInstructions(preset)">
													<ScrollText class="h-4 w-4" />
												</button>
											</RowTooltip>
											<RowTooltip v-if="preset.can_edit" :active="activeRow === preset.key" :text="store.getTranslation('presets.menu.edit')">
												<button class="p-1 rounded hover:bg-accent text-muted-foreground" data-testid="picker-preset-edit" @click.stop="editPreset(preset)">
													<Pencil class="h-4 w-4" />
												</button>
											</RowTooltip>
											<RowTooltip :active="activeRow === preset.key" :text="store.getTranslation('chat.model_selector.toggle_default')">
												<button class="p-1 rounded hover:bg-accent text-muted-foreground" @click.stop="toggleDefaultKey(preset.key)">
													<Bookmark class="h-4 w-4" :class="isDefaultKey(preset.key) ? 'fill-primary stroke-primary' : ''" />
												</button>
											</RowTooltip>
											<RowTooltip :active="activeRow === preset.key" :text="store.getTranslation('chat.model_selector.toggle_favorite')">
												<button class="p-1 rounded hover:bg-accent text-muted-foreground" @click.stop="togglePresetFavorite(preset.key)">
													<Star class="h-4 w-4" :class="isFavoriteKey(preset.key) ? 'fill-primary stroke-primary' : ''" />
												</button>
											</RowTooltip>
										</div>
									</div>
								</div>
								<p v-if="showSectionHeaders && pickerModels.length > 0" class="px-3 pt-3 pb-1 text-xs font-medium text-muted-foreground">
									{{ store.getTranslation('presets.picker.models_section') }}
								</p>
							</template>
							<div v-if="pickerFilter === 'presets' && visiblePresets.length === 0" class="p-4 text-center text-sm text-muted-foreground">
								{{ store.getTranslation('presets.picker.empty') }}
							</div>
							<div v-else-if="pickerFilter !== 'presets' && pickerModels.length === 0 && visiblePresets.length === 0" class="p-4 text-center text-sm text-muted-foreground">
								{{ store.getTranslation('chat.model_selector.no_models') }}
							</div>
							<div
								v-for="model in pickerModels"
								:key="model.id"
								data-testid="picker-model"
								class="px-3 py-2 transition-colors border-b border-border/50 last:border-0"
								:class="model.budget_blocked ? 'opacity-50 cursor-not-allowed' : 'hover:bg-accent/50 cursor-pointer'"
								@click="selectModel(model)"
								@pointerenter="activeRow = model.id"
								@focusin="activeRow = model.id"
							>
								<div class="flex items-start gap-3">
									<img v-if="model.icon" :src="model.icon" class="h-5 w-5 mt-0.5 flex items-center justify-center rounded-md bg-muted overflow-hidden" />
									<div
										v-else-if="iconStore.getProviderIcon(model.provider.name, model?.model_id)?.type === 'svg'"
										class="h-5 w-5 mt-0.5 flex items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
										v-html="providerIconHtml(model.provider.name, model?.model_id)"
									/>
									<div
										v-else-if="iconStore.getProviderIcon(model.provider.name, model?.model_id)?.type === 'png'"
										class="h-5 w-5 mt-0.5 flex items-center justify-center"
									>
										<img :src="iconStore.getProviderIcon(model.provider.name, model?.model_id)?.icon" alt="Provider icon" />
									</div>
									<Bot v-else class="h-5 w-5 mt-0.5 text-muted-foreground" />

									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2 flex-wrap">
											<span class="font-medium text-sm">{{ model.display_name }}</span>
											<span
												v-for="cap in getDisplayCapabilities(model)"
												:key="cap"
												class="px-1.5 py-0.5 text-xs rounded bg-accent text-accent-foreground"
											>
												{{ cap }}
											</span>
										</div>
										<p class="text-xs text-muted-foreground mt-0.5 truncate">
											{{ model.model_id }}
										</p>
										<p v-if="model.budget_blocked" class="text-xs text-destructive mt-1">
											{{ store.getTranslation('settings.budgets.reached') }}
										</p>
									</div>

									<div class="flex items-center gap-1 ml-2">
										<div v-if="model.capabilities.length > 0" class="p-1 gap-1 rounded-4xl bg-muted">
											<RowTooltip v-if="model.capabilities.includes('TOOLS')" :active="activeRow === model.id" :text="store.getTranslation('chat.model_selector.tool_calling')">
												<button class="rounded p-1">
													<Wrench class="h-4 w-4" />
												</button>
											</RowTooltip>
											<RowTooltip v-if="modelStore.hasReasoningCapability(model)" :active="activeRow === model.id" :text="store.getTranslation('chat.model_selector.reasoning_capable')">
												<button class="p-1 text-muted-foreground">
													<Sparkles class="h-4 w-4" />
												</button>
											</RowTooltip>
											<RowTooltip v-if="modelStore.hasVisionCapability(model)" :active="activeRow === model.id" :text="store.getTranslation('chat.model_selector.vision_capable')">
												<button class="p-1 text-muted-foreground">
													<Eye class="h-4 w-4" />
												</button>
											</RowTooltip>
										</div>
										<RowTooltip :active="activeRow === model.id" :text="store.getTranslation('chat.model_selector.toggle_default')">
											<button class="p-1 rounded hover:bg-accent text-muted-foreground" @click.stop="toggleDefault(model)">
												<Bookmark v-if="isDefaultModel(model)" class="h-4 w-4 fill-primary stroke-primary" />
												<Bookmark v-else class="h-4 w-4" />
											</button>
										</RowTooltip>
										<RowTooltip :active="activeRow === model.id" :text="store.getTranslation('chat.model_selector.toggle_favorite')">
											<button class="p-1 rounded hover:bg-accent text-muted-foreground" @click.stop="toggleFavorite(model)">
												<Star v-if="modelStore.isFavoriteModel(model)" class="h-4 w-4 fill-primary stroke-primary" />
												<Star v-else class="h-4 w-4" />
											</button>
										</RowTooltip>
									</div>
								</div>
							</div>

							<div ref="sentinel" class="h-1 flex items-center justify-center py-2">
								<Loader2 v-if="pickerLoadingMore" class="h-4 w-4 animate-spin text-muted-foreground" />
							</div>
						</template>
					</div>

					<div class="flex items-center gap-2 p-2 border-t border-border overflow-x-auto">
						<button
							class="shrink-0 w-10 h-10 flex items-center justify-center rounded-lg transition-colors"
							:class="pickerFilter === 'all' ? 'bg-primary/20 text-primary' : 'hover:bg-accent text-muted-foreground'"
							@click="pickerSelectFilter('all')"
						>
							<Bot class="h-5 w-5" />
						</button>
						<button
							class="shrink-0 w-10 h-10 flex items-center justify-center rounded-lg transition-colors"
							:class="pickerFilter === 'favorites' ? 'bg-primary/20 text-primary' : 'hover:bg-accent text-muted-foreground'"
							@click="pickerSelectFilter('favorites')"
						>
							<Star class="h-5 w-5" :class="pickerFilter === 'favorites' ? 'fill-primary' : ''" />
						</button>
						<button
							data-testid="picker-presets-tab"
							class="shrink-0 w-10 h-10 flex items-center justify-center rounded-lg transition-colors"
							:class="pickerFilter === 'presets' ? 'bg-primary/20 text-primary' : 'hover:bg-accent text-muted-foreground'"
							:title="store.getTranslation('presets.picker.tab')"
							@click="pickerSelectFilter('presets')"
						>
							<Wand2 class="h-5 w-5" />
						</button>
						<div class="shrink-0 w-px h-6 bg-border" />
						<button
							v-for="provider in pickerProviders"
							:key="provider.id"
							class="shrink-0 w-10 h-10 flex items-center justify-center rounded-lg transition-colors"
							:class="pickerFilter === provider.id ? 'bg-primary/20 text-primary' : 'hover:bg-accent text-muted-foreground'"
							:title="provider.name"
							@click="pickerSelectFilter(provider.id)"
						>
							<div
								v-if="iconStore.getProviderIcon(provider.name)?.type === 'svg'"
								class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full"
								v-html="providerIconHtml(provider.name)"
							/>
							<div v-else-if="iconStore.getProviderIcon(provider.name)?.type === 'png'" class="h-5 w-5 flex items-center justify-center">
								<img :src="iconStore.getProviderIcon(provider.name)?.icon" alt="Provider icon" />
							</div>
							<Bot v-else class="h-5 w-5" />
						</button>
						<div class="flex-1" />
						<button
							v-if="canCreatePresets"
							type="button"
							data-testid="picker-save-preset"
							class="shrink-0 inline-flex items-center gap-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
							:title="store.getTranslation('presets.picker.save_hint')"
							@click="savePreset"
						>
							<BookmarkPlus class="h-3.5 w-3.5" />
							{{ store.getTranslation('presets.menu.save_as') }}
						</button>
					</div>
				</div>
			</div>
		</PopoverContent>
	</Popover>

	<ShadDialog :open="!!instructionsPreset" @update:open="open => !open && (instructionsPreset = null)">
		<ShadDialogContent class="max-w-2xl">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('presets.menu.instructions_title', {preset: instructionsPreset?.name ?? ''}) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('presets.menu.instructions_description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<pre class="max-h-[60vh] overflow-y-auto whitespace-pre-wrap rounded-lg bg-muted p-3 font-mono text-xs text-foreground" data-testid="preset-instructions">{{ instructionsPreset?.system_prompt }}</pre>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Bot, Star, Bookmark, BookmarkPlus, ChevronDown, Search, Filter, Wrench, Sparkles, Eye, Loader2, Pencil, ScrollText, Wand2} from 'lucide-vue-next';
import type {ModelList, UpdatePreferencesRequest} from '~/types/chat';
import type {PresetResponse} from '~/types/presets';
import {usePresetStore} from '~/stores/presetStore';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {usePreferencesStore} from '~/stores/preferencesStore';
import {useMainStore} from '~/stores';
import {useModelPicker} from '~/composables/useModelPicker';
import {sanitizeSvg} from '~/composables/useMarkdown';
import {cn} from '~/lib/utils';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import RowTooltip from './RowTooltip.vue';

const props = defineProps<{
	class?: string;
}>();

const chatStore = useChatStore();
const modelStore = useModelStore();
const presetStore = usePresetStore();
const preferencesStore = usePreferencesStore();
const iconStore = useIconsStore();
const store = useMainStore();

const sanitizedIcons = new Map<string, string>();

function providerIconHtml(providerName: string | undefined, modelId?: string): string {
	const icon = iconStore.getProviderIcon(providerName, modelId);
	if (icon?.type !== 'svg') return '';
	let html = sanitizedIcons.get(icon.icon);
	if (html === undefined) {
		html = sanitizeSvg(icon.icon);
		sanitizedIcons.set(icon.icon, html);
	}
	return html;
}
const fullName = computed(() => modelStore.selectedModel?.display_name || store.getTranslation('chat.model_selector.select_model'));
const shortName = computed(() => {
	const model = modelStore.selectedModel;
	if (!model || model.preset) return fullName.value;
	return model.display_name.replace(/^[^:]+:\s+/, '') || model.display_name;
});

const isOpen = ref(false);
const activeRow = ref<string | null>(null);
const {
	models: pickerModels,
	providers: pickerProviders,
	loading: pickerLoading,
	loadingMore: pickerLoadingMore,
	searchQuery: pickerSearch,
	selectedFilter: pickerFilter,
	canLoadMore: pickerCanLoadMore,
	loadMore: pickerLoadMore,
	onSearchInput: pickerOnSearch,
	clearSearch: pickerClearSearch,
	selectFilter: pickerSelectFilter,
	init: pickerInit,
} = useModelPicker();

const scrollContainer = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const activeChatId = computed(() => chatStore.activeChat?.id);

let pickerLoaded = false;
watch(() => modelStore.models, () => (pickerLoaded = false));

watch(isOpen, async (open) => {
	if (open) {
		presetStore.ensurePresets();
		if (!pickerLoaded) {
			pickerLoaded = true;
			await pickerInit();
		}
	} else {
		activeRow.value = null;
		if (pickerClearSearch()) pickerLoaded = false;
	}
});

watch(sentinel, element => (element ? setupObserver() : teardownObserver()));

function setupObserver() {
	teardownObserver();
	if (!sentinel.value || !scrollContainer.value) return;
	observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (entry.isIntersecting && pickerCanLoadMore.value) {
					pickerLoadMore();
				}
			}
		},
		{root: scrollContainer.value, rootMargin: '50px'},
	);
	observer.observe(sentinel.value);
}

function teardownObserver() {
	if (observer) {
		observer.disconnect();
		observer = null;
	}
}

onUnmounted(() => {
	teardownObserver();
});

const {canCreate: canCreatePresets, openSettings, saveAsPreset} = usePresetActions();
const instructionsPreset = ref<PresetResponse | null>(null);

function savePreset() {
	isOpen.value = false;
	saveAsPreset();
}

function editPreset(preset: PresetResponse) {
	isOpen.value = false;
	openSettings(`/settings/presets/${preset.id}`);
}

function showInstructions(preset: PresetResponse) {
	isOpen.value = false;
	instructionsPreset.value = preset;
}

const visiblePresets = computed(() => {
	const filter = pickerFilter.value;
	if (filter !== 'all' && filter !== 'presets' && filter !== 'favorites') return [];
	const needle = pickerSearch.value.trim().toLowerCase();
	return presetStore.presets.filter(preset => {
		if (filter === 'favorites' && !isFavoriteKey(preset.key)) return false;
		if (!needle) return true;
		return `${preset.name} ${preset.key} ${preset.description ?? ''}`.toLowerCase().includes(needle);
	});
});

const showSectionHeaders = computed(() => pickerFilter.value !== 'presets');

function isPresetBlocked(preset: PresetResponse): boolean {
	return !!preset.unavailable || !!preset.base_model?.budget_blocked;
}

function presetProblem(preset: PresetResponse): string | null {
	if (preset.unavailable) return store.getTranslation(`presets.unavailable.${preset.unavailable}`);
	if (preset.base_model?.budget_blocked) return store.getTranslation('settings.budgets.reached');
	return null;
}

function selectPreset(preset: PresetResponse) {
	const problem = presetProblem(preset);
	if (problem) {
		store.toast(problem, {type: 'warning'});
		return;
	}
	const model = presetAsModel(preset);
	if (!model) return;
	modelStore.setSelectedModel(model);
	isOpen.value = false;
}

function isFavoriteKey(key: string): boolean {
	return store.preferences?.favorite_model_keys?.includes(key) ?? false;
}

async function togglePresetFavorite(key: string) {
	const current = store.preferences?.favorite_model_keys ?? [];
	const next = current.includes(key) ? current.filter(k => k !== key) : [...current, key];
	await preferencesStore.updatePreferences({favorite_model_keys: next});
}

function isDefaultKey(key: string): boolean {
	return store.preferences?.default_model_key === key;
}

async function toggleDefaultKey(key: string) {
	await preferencesStore.updatePreferences({default_model_key: isDefaultKey(key) ? null : key, default_provider_slug: null});
}

function getDisplayCapabilities(model: ModelList): string[] {
	const caps: string[] = [];
	if (model.context_length) {
		caps.push(`${Math.round(model.context_length / 1000)}k`);
	}
	return caps;
}

function selectModel(model: ModelList) {
	if (model.budget_blocked) {
		store.toast(store.getTranslation('settings.budgets.reached'), {type: 'warning'});
		return;
	}
	modelStore.setSelectedModel(model);
	isOpen.value = false;
}

function toggleFavorite(model: ModelList) {
	const newVal = !model.is_favorite;
	const idx = pickerModels.value.findIndex(m => m.id === model.id);
	if (idx !== -1) pickerModels.value[idx] = {...pickerModels.value[idx], is_favorite: newVal};
	modelStore.toggleFavoriteModel(model.id);
}

function isDefaultModel(model: ModelList): boolean {
	return store.preferences?.default_model_key === model.model_id;
}

async function toggleDefault(model: ModelList) {
	const wasDefault = isDefaultModel(model);
	const newKey = wasDefault ? null : model.model_id;
	const update: UpdatePreferencesRequest = {default_model_key: newKey};

	if (wasDefault) {
		update.default_provider_slug = null;
	} else {
		const currentProviderSlug = store.preferences?.default_provider_slug ?? null;
		if (currentProviderSlug) {
			let available = false;
			let checkedAvailability = false;
			try {
				const res = await modelProviderOptions<{options?: {provider_slug: string | null}[]}>(model.id);
				available = (res.options ?? []).some(o => (o.provider_slug ?? '') === currentProviderSlug);
				checkedAvailability = true;
			} catch (e) {
				console.error('Failed to check provider availability for default model:', e);
			}
			if (checkedAvailability && !available) {
				update.default_provider_slug = null;
			}
		}
	}

	await preferencesStore.updatePreferences(update);
}

</script>
