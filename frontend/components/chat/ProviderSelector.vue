<template>
	<Popover v-if="isVisible" v-model:open="isOpen">
		<PopoverTrigger as-child>
			<button
				:class="
					cn(
						'flex items-center gap-2 px-2 py-1 rounded-md hover:bg-accent transition-colors text-xs',
						chatStore.selectedProviderSlug ? 'text-primary' : 'text-muted-foreground',
						props.class,
					)
				"
			>
				<template v-if="chatStore.selectedProviderSlug">
					<div
						v-if="triggerIcon?.type === 'svg'"
						class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full"
						v-html="triggerIcon.icon"
					/>
					<img v-else-if="triggerIcon?.type === 'png'" :src="triggerIcon.icon" class="h-4 w-4" alt="" />
					<Server v-else class="h-4 w-4" />
				</template>
				<Server v-else class="h-4 w-4" />
				<span class="max-w-32 truncate font-medium">{{ selectedLabel }}</span>
				<ChevronDown class="h-3 w-3 text-muted-foreground" />
			</button>
		</PopoverTrigger>
		<PopoverContent class="w-[560px] p-0 overflow-hidden" align="start" :side-offset="8">
			<div class="flex h-[380px]">
				<!-- Left: provider list -->
				<div class="w-[240px] flex flex-col border-r border-border shrink-0">
					<!-- Search -->
					<div class="relative border-b border-border">
						<Search class="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
						<input
							v-model="searchQuery"
							:placeholder="store.getTranslation('chat.provider_selector.search_placeholder')"
							class="w-full pl-9 pr-10 py-2.5 text-sm bg-transparent outline-none placeholder:text-muted-foreground"
						/>
						<div class="absolute right-3 top-1/2 -translate-y-1/2">
							<button class="text-muted-foreground hover:text-foreground transition-colors">
								<Filter class="h-3.5 w-3.5" />
							</button>
						</div>
					</div>

					<!-- Provider list -->
					<div class="flex-1 overflow-y-auto">
						<div v-if="loading" class="flex items-center justify-center py-8 text-muted-foreground">
							<Loader2 class="h-5 w-5 animate-spin" />
						</div>
						<template v-else>
							<!-- Automatic -->
							<button
								class="w-full px-3 py-2.5 flex items-center gap-3 transition-colors text-left"
								:class="!chatStore.selectedProviderSlug && hoveredOpt === null ? 'bg-accent/70' : 'hover:bg-accent/40'"
								@mouseenter="hoveredOpt = null; isHoveringAuto = true"
								@mouseleave="isHoveringAuto = false"
								@click="selectAuto"
							>
								<div class="h-8 w-8 rounded-lg bg-muted flex items-center justify-center shrink-0">
									<Sparkles class="h-4 w-4 text-muted-foreground" />
								</div>
								<div class="flex-1 min-w-0">
									<div class="text-sm font-medium">{{ store.getTranslation('chat.provider_selector.automatic') }}</div>
									<div class="text-xs text-muted-foreground truncate">{{ store.getTranslation('chat.provider_selector.automatic_hint') }}</div>
								</div>
								<Check v-if="!chatStore.selectedProviderSlug" class="h-4 w-4 text-primary shrink-0" />
							</button>

							<div v-if="parentUnavailable" class="p-4 text-center text-xs text-muted-foreground">
								{{ store.getTranslation('chat.provider_selector.unavailable_for_key') }}
							</div>
							<template v-else-if="sortedOptions.length > 0">
								<button
									v-for="opt in sortedOptions"
									:key="opt.id"
									class="w-full px-3 py-2.5 flex items-center gap-3 transition-colors text-left"
									:class="[
										rowDisabled(opt) ? 'opacity-40 pointer-events-none' : '',
										(hoveredOpt?.id === opt.id || (isSelected(opt) && hoveredOpt === null && !isHoveringAuto)) ? 'bg-accent/70' : 'hover:bg-accent/40',
									]"
									@mouseenter="hoveredOpt = opt; isHoveringAuto = false"
									@mouseleave="hoveredOpt = null"
									@click="selectOption(opt)"
								>
									<div class="h-8 w-8 rounded-lg bg-muted flex items-center justify-center shrink-0 overflow-hidden">
										<div
											v-if="getProviderIconData(opt.provider_name, opt.provider_slug)?.type === 'svg'"
											class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full"
											v-html="getProviderIconData(opt.provider_name, opt.provider_slug)?.icon"
										/>
										<img
											v-else-if="getProviderIconData(opt.provider_name, opt.provider_slug)?.type === 'png'"
											:src="getProviderIconData(opt.provider_name, opt.provider_slug)?.icon"
											class="h-4 w-4"
											alt=""
										/>
										<Bot v-else class="h-3.5 w-3.5 text-muted-foreground" />
									</div>
									<div class="flex-1 min-w-0">
										<div class="text-sm font-medium truncate">{{ opt.provider_name || opt.provider_slug || '—' }}</div>
										<div class="text-xs text-muted-foreground truncate">{{ opt.provider_slug }}</div>
									</div>
									<div class="h-2 w-2 rounded-full shrink-0" :class="getStatusColor(opt)" />
								</button>
							</template>
							<div v-else class="p-4 text-center text-xs text-muted-foreground">
								{{ store.getTranslation('chat.provider_selector.no_providers') }}
							</div>
						</template>
					</div>
				</div>

				<!-- Right: details panel -->
				<div class="flex-1 flex flex-col overflow-hidden">
					<template v-if="detailOpt === null">
						<!-- Automatic header -->
						<div class="flex items-center gap-2 px-4 pt-4 pb-3 border-b border-border/50">
							<Sparkles class="h-4 w-4 text-muted-foreground shrink-0" />
							<span class="font-semibold text-sm">{{ store.getTranslation('chat.provider_selector.automatic') }}</span>
						</div>
						<div class="flex-1 overflow-y-auto px-4 py-3 space-y-2.5">
							<template v-if="autoDetails">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-1.5 text-muted-foreground">
										<Tag class="h-3 w-3" />
										<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_price') }}</span>
									</div>
									<div class="text-right">
										<span class="text-xs font-medium tabular-nums">{{ formatPrice(autoDetails.price_input) }} / {{ formatPrice(autoDetails.price_output) }}</span>
										<span class="text-[10px] text-muted-foreground ml-1">{{ store.getTranslation('chat.provider_selector.price_io_label') }}</span>
									</div>
								</div>
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-1.5 text-muted-foreground">
										<Cpu class="h-3 w-3" />
										<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_context') }}</span>
									</div>
									<span class="text-xs font-medium tabular-nums">{{ formatContextFull(autoDetails.context_length) }} {{ store.getTranslation('chat.provider_selector.context_tokens') }}</span>
								</div>
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-1.5 text-muted-foreground">
										<Clock class="h-3 w-3" />
										<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_latency') }}</span>
									</div>
									<div class="text-right">
										<span class="text-xs font-medium tabular-nums">{{ formatLatency(autoDetails.latency) }}</span>
										<span class="text-[10px] text-muted-foreground ml-1">{{ store.getTranslation('chat.provider_selector.latency_estimated') }}</span>
									</div>
								</div>
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-1.5 text-muted-foreground">
										<Gauge class="h-3 w-3" />
										<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_throughput') }}</span>
									</div>
									<span class="text-xs font-medium tabular-nums">{{ formatThroughput(autoDetails.throughput) }}</span>
								</div>
							</template>
						</div>
					</template>

					<template v-else>
						<!-- Provider header -->
						<div class="flex items-center gap-2 px-4 pt-4 pb-3 border-b border-border/50">
							<div class="h-4 w-4 flex items-center justify-center shrink-0 overflow-hidden">
								<div
									v-if="getProviderIconData(detailOpt.provider_name, detailOpt.provider_slug)?.type === 'svg'"
									class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full"
									v-html="getProviderIconData(detailOpt.provider_name, detailOpt.provider_slug)?.icon"
								/>
								<img
									v-else-if="getProviderIconData(detailOpt.provider_name, detailOpt.provider_slug)?.type === 'png'"
									:src="getProviderIconData(detailOpt.provider_name, detailOpt.provider_slug)?.icon"
									class="h-4 w-4"
									alt=""
								/>
								<Bot v-else class="h-4 w-4 text-muted-foreground" />
							</div>
							<span class="font-semibold text-sm truncate">{{ detailOpt.provider_name || detailOpt.provider_slug }}</span>
						</div>
						<div class="flex-1 overflow-y-auto px-4 py-3 space-y-2.5">
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-1.5 text-muted-foreground">
									<Tag class="h-3 w-3" />
									<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_price') }}</span>
								</div>
								<div class="text-right">
									<span class="text-xs font-medium tabular-nums">{{ formatPrice(detailOpt.price_input) }} / {{ formatPrice(detailOpt.price_output) }}</span>
									<span class="text-[10px] text-muted-foreground ml-1">{{ store.getTranslation('chat.provider_selector.price_io_label') }}</span>
								</div>
							</div>
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-1.5 text-muted-foreground">
									<Cpu class="h-3 w-3" />
									<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_context') }}</span>
								</div>
								<span class="text-xs font-medium tabular-nums">{{ formatContextFull(detailOpt.context_length) }} {{ store.getTranslation('chat.provider_selector.context_tokens') }}</span>
							</div>
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-1.5 text-muted-foreground">
									<Clock class="h-3 w-3" />
									<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_latency') }}</span>
								</div>
								<div class="text-right">
									<span class="text-xs font-medium tabular-nums">{{ formatLatency(detailOpt.latency) }}</span>
									<span class="text-[10px] text-muted-foreground ml-1">{{ store.getTranslation('chat.provider_selector.latency_estimated') }}</span>
								</div>
							</div>
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-1.5 text-muted-foreground">
									<Gauge class="h-3 w-3" />
									<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_throughput') }}</span>
								</div>
								<span class="text-xs font-medium tabular-nums">{{ formatThroughput(detailOpt.throughput) }}</span>
							</div>
							<div class="flex items-center justify-between">
								<div class="flex items-center gap-1.5 text-muted-foreground">
									<Activity class="h-3 w-3" />
									<span class="text-xs">{{ store.getTranslation('chat.provider_selector.col_uptime') }}</span>
								</div>
								<span class="text-xs font-medium tabular-nums">{{ formatUptime(detailOpt.uptime) }}</span>
							</div>
						</div>
					</template>
				</div>
			</div>
		</PopoverContent>
	</Popover>
</template>

<script setup lang="ts">
import {Server, ChevronDown, Check, Loader2, Sparkles, Search, Filter, Bot, Tag, Cpu, Clock, Gauge, Activity} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {useIconsStore} from '~/stores/icons';
import {cn} from '~/lib/utils';
import {Popover, PopoverContent, PopoverTrigger} from '~/components/ui/popover';

interface ProviderOption {
	id: string;
	provider_slug: string | null;
	provider_name: string | null;
	endpoint_name: string | null;
	status: number | null;
	quantization: string | null;
	context_length: number | null;
	max_completion_tokens: number | null;
	latency: number | null;
	throughput: number | null;
	uptime: number | null;
	price_input: number | null;
	price_output: number | null;
}

interface ProviderOptionsResponse {
	gateway_model_id: string | null;
	availability_state: 'AVAILABLE' | 'USER_UNAVAILABLE' | null;
	options: ProviderOption[];
}



const props = defineProps<{class?: string}>();

const chatStore = useChatStore();
const store = useMainStore();
const iconStore = useIconsStore();

const isOpen = ref(false);
const loading = ref(false);
const options = ref<ProviderOption[]>([]);
const parentUnavailable = ref(false);
const loadedModelId = ref<string | null>(null);

const searchQuery = ref('');

const hoveredOpt = ref<ProviderOption | null>(null);
const isHoveringAuto = ref(false);

function recommendScore(opt: ProviderOption): number {
	const operational = opt.status == null || opt.status >= 0 ? 1 : 0;
	const uptime = (opt.uptime ?? 50) / 100;
	const latencyScore = opt.latency != null ? Math.max(0, 1 - opt.latency / 5000) : 0.5;
	const throughputScore = opt.throughput != null ? Math.min(opt.throughput / 300, 1) : 0.5;
	return operational * (uptime * 0.4 + latencyScore * 0.35 + throughputScore * 0.25);
}

const filteredOptions = computed(() => {
	if (!searchQuery.value.trim()) return options.value;
	const q = searchQuery.value.toLowerCase();
	return options.value.filter(o => (o.provider_name ?? '').toLowerCase().includes(q) || (o.provider_slug ?? '').toLowerCase().includes(q));
});

const sortedOptions = computed(() => {
	const list = [...filteredOptions.value];
	list.sort((a, b) => recommendScore(b) - recommendScore(a));
	return list;
});

const detailOpt = computed<ProviderOption | null>(() => {
	if (hoveredOpt.value) return hoveredOpt.value;
	if (isHoveringAuto.value) return null;
	if (chatStore.selectedProviderSlug) {
		return options.value.find(o => (o.provider_slug || '') === chatStore.selectedProviderSlug) ?? null;
	}
	return null;
});

const autoDetails = computed(() => sortedOptions.value[0] ?? null);

const isVisible = computed(() => {
	return (store.base?.enable_provider_selector ?? false) && chatStore.selectedModel?.provider?.kind === 'OPENROUTER';
});

const selectedLabel = computed(() => {
	if (!chatStore.selectedProviderSlug) return store.getTranslation('chat.provider_selector.automatic');
	const match = options.value.find(o => (o.provider_slug || '') === chatStore.selectedProviderSlug);
	return match?.provider_name || chatStore.selectedProviderSlug;
});

const triggerIcon = computed(() => {
	if (!chatStore.selectedProviderSlug) return null;
	const match = options.value.find(o => (o.provider_slug || '') === chatStore.selectedProviderSlug);
	return iconStore.getProviderIcon(match?.provider_name ?? chatStore.selectedProviderSlug, undefined);
});


function rowDisabled(opt: ProviderOption): boolean {
	return opt.status != null && opt.status < 0;
}

function isSelected(opt: ProviderOption): boolean {
	return !!chatStore.selectedProviderSlug && (opt.provider_slug || '') === chatStore.selectedProviderSlug;
}

function isProviderHealthy(opt: ProviderOption): boolean {
	if (opt.status != null && opt.status < 0) return false;
	if (opt.uptime != null && opt.uptime < 95) return false;
	return true;
}

function getStatusColor(opt: ProviderOption): string {
	if (opt.status == null) return 'bg-green-500';
	if (opt.status < 0) return 'bg-red-500';
	if (opt.uptime != null && opt.uptime < 95) return 'bg-yellow-500';
	return 'bg-green-500';
}

function getProviderIconData(name: string | null, slug: string | null) {
	return iconStore.getProviderIcon(name ?? slug ?? '', undefined);
}

function formatPrice(value: number | null): string {
	if (value == null) return '—';
	return `$${value.toFixed(2)}`;
}

function formatLatency(value: number | null): string {
	if (value == null) return '—';
	return `${Math.round(value)}ms`;
}

function formatThroughput(value: number | null): string {
	if (value == null) return '—';
	return `${Math.round(value)} t/s`;
}

function formatUptime(value: number | null): string {
	if (value == null) return '—';
	return `${value.toFixed(1)}%`;
}

function formatContextFull(value: number | null): string {
	if (value == null) return '—';
	if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(0)}M`;
	if (value >= 1_000) return `${(value / 1_000).toFixed(0)}K`;
	return String(value);
}

function selectAuto() {
	chatStore.setProviderSelection(null);
	isOpen.value = false;
}

function selectOption(opt: ProviderOption) {
	if (!opt.provider_slug) return;
	chatStore.setProviderSelection(opt.provider_slug, chatStore.providerRoutingMode);
	isOpen.value = false;
}

async function loadOptions() {
	const model = chatStore.selectedModel;
	if (!model) return;
	if (loadedModelId.value === model.id && options.value.length > 0) return;

	loading.value = true;
	parentUnavailable.value = false;
	try {
		const {$customFetch} = useNuxtApp();
		const res = await $customFetch<ProviderOptionsResponse>(`/api/v1/models/${model.id}/provider-options`);
		options.value = res.options ?? [];
		parentUnavailable.value = res.availability_state === 'USER_UNAVAILABLE';
		loadedModelId.value = model.id;
	} catch (error) {
		console.error('Failed to load provider options:', error);
		options.value = [];
	} finally {
		loading.value = false;
	}
}

watch(isOpen, open => {
	if (open) {
		loadOptions();
	} else {
		hoveredOpt.value = null;
		isHoveringAuto.value = false;
	}
});
</script>
