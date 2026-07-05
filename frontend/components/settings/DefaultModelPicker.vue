<template>
	<Popover v-model:open="isOpen">
		<PopoverTrigger as-child>
			<button
				:disabled="disabled"
				class="flex items-center gap-2 px-3 py-2 rounded-md border border-input bg-background hover:bg-accent transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed w-full max-w-sm"
			>
				<template v-if="selectedModel">
					<img v-if="selectedModel.icon" :src="selectedModel.icon" class="h-4 w-4 rounded object-cover shrink-0" />
					<div
						v-else-if="iconStore.getProviderIcon(selectedModel.provider?.name, selectedModel.model_id)?.type === 'svg'"
						class="h-4 w-4 shrink-0 flex items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
						v-html="iconStore.getProviderIcon(selectedModel.provider?.name, selectedModel.model_id)?.icon"
					/>
					<Bot v-else class="h-4 w-4 shrink-0 text-muted-foreground" />
					<span class="flex-1 truncate text-left">{{ selectedModel.display_name }}</span>
				</template>
				<template v-else>
					<Bot class="h-4 w-4 shrink-0 text-muted-foreground" />
					<span class="flex-1 truncate text-left text-muted-foreground">{{ effectivePlaceholder }}</span>
				</template>
				<ChevronDown class="h-4 w-4 shrink-0 text-muted-foreground" />
			</button>
		</PopoverTrigger>

		<PopoverContent class="w-130 p-0 overflow-hidden" align="start" :side-offset="8">
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
					</div>

					<div ref="scrollContainer" class="flex-1 overflow-y-auto">
						<div
							class="px-3 py-2 hover:bg-accent/50 cursor-pointer transition-colors border-b border-border/50 flex items-center gap-3 text-sm text-muted-foreground"
							@click="selectModel(null)"
						>
							<X class="h-4 w-4 shrink-0" />
							<span>{{ effectivePlaceholder }}</span>
						</div>

						<div v-if="pickerLoading" class="flex items-center justify-center py-8 text-muted-foreground">
							<Loader2 class="h-5 w-5 animate-spin" />
						</div>
						<template v-else>
							<div v-if="pickerModels.length === 0" class="p-4 text-center text-sm text-muted-foreground">
								{{ store.getTranslation('chat.model_selector.no_models') }}
							</div>
							<div
								v-for="model in pickerModels"
								:key="model.id"
								class="px-3 py-2 hover:bg-accent/50 cursor-pointer transition-colors border-b border-border/50 last:border-0"
								:class="model.model_id === modelValue ? 'bg-accent/30' : ''"
								@click="selectModel(model)"
							>
								<div class="flex items-start gap-3">
									<img v-if="model.icon" :src="model.icon" class="h-5 w-5 mt-0.5 rounded object-cover shrink-0" />
									<div
										v-else-if="iconStore.getProviderIcon(model.provider?.name, model.model_id)?.type === 'svg'"
										class="h-5 w-5 mt-0.5 shrink-0 flex items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
										v-html="iconStore.getProviderIcon(model.provider?.name, model.model_id)?.icon"
									/>
									<Bot v-else class="h-5 w-5 mt-0.5 shrink-0 text-muted-foreground" />

									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2 flex-wrap">
											<span class="font-medium text-sm">{{ model.display_name }}</span>
											<span v-if="model.context_length" class="px-1.5 py-0.5 text-xs rounded bg-accent text-accent-foreground">
												{{ Math.round(model.context_length / 1000) }}k
											</span>
										</div>
										<p class="text-xs text-muted-foreground mt-0.5 truncate">{{ model.model_id }}</p>
									</div>

									<Check v-if="model.model_id === modelValue" class="h-4 w-4 shrink-0 text-primary mt-0.5" />
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
								v-html="iconStore.getProviderIcon(provider.name)?.icon"
							/>
							<div v-else-if="iconStore.getProviderIcon(provider.name)?.type === 'png'" class="h-5 w-5 flex items-center justify-center">
								<img :src="iconStore.getProviderIcon(provider.name)?.icon" alt="Provider icon" />
							</div>
							<Bot v-else class="h-5 w-5" />
						</button>
					</div>
				</div>
			</div>
		</PopoverContent>
	</Popover>
</template>

<script setup lang="ts">
import {ref, computed, watch, onUnmounted} from 'vue';
import {Bot, ChevronDown, Check, Loader2, Search, X} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {useModelPicker} from '~/composables/useModelPicker';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import type {ModelList} from '~/types/chat';

const props = withDefaults(defineProps<{
	modelValue: string | null;
	endpoint?: string;
	disabled?: boolean;
	placeholder?: string;
}>(), {
	endpoint: '/api/v1/models',
	disabled: false,
	placeholder: undefined,
});

const emit = defineEmits<{
	'update:modelValue': [value: string | null];
}>();

const store = useMainStore();
const iconStore = useIconsStore();
const isOpen = ref(false);

const effectivePlaceholder = computed(() => props.placeholder ?? store.getTranslation('settings.teams.use_global_default'));

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
	selectFilter: pickerSelectFilter,
	init: pickerInit,
} = useModelPicker({endpoint: props.endpoint});

const scrollContainer = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const selectedModel = computed<ModelList | null>(() => {
	if (!props.modelValue) return null;
	return pickerModels.value.find(m => m.model_id === props.modelValue) ?? null;
});

watch(isOpen, async open => {
	if (open) {
		await pickerInit();
		setupObserver();
	} else {
		teardownObserver();
	}
});

function setupObserver() {
	teardownObserver();
	if (!sentinel.value || !scrollContainer.value) return;
	observer = new IntersectionObserver(
		entries => {
			for (const entry of entries) {
				if (entry.isIntersecting && pickerCanLoadMore.value) pickerLoadMore();
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

onUnmounted(teardownObserver);

function selectModel(model: ModelList | null) {
	emit('update:modelValue', model?.model_id ?? null);
	isOpen.value = false;
}
</script>
