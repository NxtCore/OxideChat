<template>
	<Popover v-model:open="isOpen">
		<PopoverTrigger as-child>
			<button :class="cn('flex items-center gap-2 px-2 py-1 rounded-md hover:bg-accent transition-colors', props.class)">
				<img
					v-if="chatStore.selectedModel?.icon"
					:src="chatStore.selectedModel?.icon"
					class="h-4 w-4 mt-0.5 flex items-center justify-center rounded-md bg-muted overflow-hidden"
				/>
				<div
					v-else-if="iconStore.getProviderIcon(chatStore.selectedModel?.provider.name, chatStore.selectedModel?.model_id)?.type === 'svg'"
					class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
					v-html="iconStore.getProviderIcon(chatStore.selectedModel?.provider.name, chatStore.selectedModel?.model_id)?.icon"
				/>
				<div
					v-else-if="iconStore.getProviderIcon(chatStore.selectedModel?.provider.name, chatStore.selectedModel?.model_id)?.type === 'png'"
					class="h-4 w-4 flex items-center justify-center"
				>
					<img :src="iconStore.getProviderIcon(chatStore.selectedModel?.provider.name, chatStore.selectedModel?.model_id)?.icon" alt="Provider icon" />
				</div>
				<span class="max-w-37.5 truncate text-xs font-medium">
					{{ chatStore.selectedModel?.display_name || store.getTranslation('chat.model_selector.select_model') }}
				</span>
				<ChevronDown class="h-3 w-3 text-muted-foreground" />
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
						<button class="p-1.5 rounded-md hover:bg-accent text-muted-foreground">
							<Filter class="h-4 w-4" />
						</button>
					</div>

					<div ref="scrollContainer" class="flex-1 overflow-y-auto">
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
								@click="selectModel(model)"
							>
								<div class="flex items-start gap-3">
									<img v-if="model.icon" :src="model.icon" class="h-5 w-5 mt-0.5 flex items-center justify-center rounded-md bg-muted overflow-hidden" />
									<div
										v-else-if="iconStore.getProviderIcon(model.provider.name, model?.model_id)?.type === 'svg'"
										class="h-5 w-5 mt-0.5 flex items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
										v-html="iconStore.getProviderIcon(model.provider.name, model?.model_id)?.icon"
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
											<Star v-if="model.is_favorite" class="h-3.5 w-3.5 text-yellow-500 fill-yellow-500" />
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
									</div>

									<div class="flex items-center gap-1 ml-2">
										<div v-if="model.capabilities.length > 0" class="p-1 gap-1 rounded-4xl bg-muted">
											<Tooltip v-if="model.capabilities.includes('TOOLS')">
												<TooltipTrigger as-child>
													<button class="rounded p-1">
														<Wrench class="h-4 w-4" />
													</button>
												</TooltipTrigger>
												<TooltipContent>
													<p>{{ store.getTranslation('chat.model_selector.tool_calling') }}</p>
												</TooltipContent>
											</Tooltip>
											<Tooltip v-if="chatStore.hasReasoningCapability(model)">
												<TooltipTrigger as-child>
													<button class="p-1 text-muted-foreground">
														<Sparkles class="h-4 w-4" />
													</button>
												</TooltipTrigger>
												<TooltipContent>
													<p>{{ store.getTranslation('chat.model_selector.reasoning_capable') }}</p>
												</TooltipContent>
											</Tooltip>
											<Tooltip v-if="chatStore.hasVisionCapability(model)">
												<TooltipTrigger as-child>
													<button class="p-1 text-muted-foreground">
														<Eye class="h-4 w-4" />
													</button>
												</TooltipTrigger>
												<TooltipContent>
													<p>{{ store.getTranslation('chat.model_selector.vision_capable') }}</p>
												</TooltipContent>
											</Tooltip>
										</div>
										<Tooltip>
											<TooltipTrigger as-child>
												<button class="p-1 rounded hover:bg-accent text-muted-foreground" @click.stop="toggleFavorite(model)">
													<Star v-if="chatStore.isFavoriteModel(model)" class="h-4 w-4 fill-primary stroke-primary" />
													<Star v-else class="h-4 w-4" />
												</button>
											</TooltipTrigger>
											<TooltipContent>
												<p>{{ store.getTranslation('chat.model_selector.toggle_favorite') }}</p>
											</TooltipContent>
										</Tooltip>
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
import {Bot, Star, ChevronDown, Search, Filter, Wrench, Sparkles, Eye, Loader2} from 'lucide-vue-next';
import type {Model} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {useModelPicker} from '~/composables/useModelPicker';
import {cn} from '~/lib/utils';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';
import {Tooltip, TooltipContent, TooltipTrigger} from '~/components/ui/tooltip';

const props = defineProps<{
	class?: string;
}>();

const chatStore = useChatStore();
const iconStore = useIconsStore();
const store = useMainStore();
const isOpen = ref(false);
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
} = useModelPicker();

const scrollContainer = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const activeChatId = computed(() => chatStore.activeChat?.id);

watch(isOpen, async (open) => {
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

function getDisplayCapabilities(model: Model): string[] {
	const caps: string[] = [];
	if (model.context_length) {
		caps.push(`${Math.round(model.context_length / 1000)}k`);
	}
	return caps;
}

function selectModel(model: Model) {
	chatStore.setSelectedModel(model);
	isOpen.value = false;
}

function toggleFavorite(model: Model) {
	chatStore.toggleFavoriteModel(model.model_id);
}

watch(
	() => [chatStore.activeChat?.id, chatStore.models.length],
	(newVal, oldVal) => {
		if (oldVal !== newVal && newVal && chatStore.messages.length > 0) {
			const last_llm_message = chatStore.messages.findLast(m => m.role === 'assistant');
			const model = chatStore.models.find(m => m.id === last_llm_message?.model_id);
			if (model) {
				chatStore.setSelectedModel(model);
			}
		}
	}
);
</script>
