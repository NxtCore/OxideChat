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
			<!-- Search -->
			<div class="relative border-b border-border">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
				<input
					v-model="searchQuery"
					:placeholder="store.getTranslation('chat.provider_selector.search_placeholder')"
					class="w-full pl-9 pr-4 py-2.5 text-sm bg-transparent outline-none placeholder:text-muted-foreground"
				/>
			</div>

			<!-- Sort -->
			<div class="flex items-center justify-end px-3 py-2 border-b border-border">
				<div class="relative">
					<button
						class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
						@click="sortOpen = !sortOpen"
					>
						{{ store.getTranslation('chat.provider_selector.sort_label') }}: {{ currentSortLabel }}
						<ChevronDown class="h-3 w-3 transition-transform" :class="sortOpen ? 'rotate-180' : ''" />
					</button>
					<div v-if="sortOpen" class="absolute right-0 top-6 bg-popover border border-border rounded-md shadow-md z-50 min-w-36">
						<button
							v-for="s in sortOptions"
							:key="s.key"
							class="w-full text-left px-3 py-1.5 text-xs hover:bg-accent transition-colors"
							:class="sortBy === s.key ? 'text-primary' : 'text-foreground'"
							@click="sortBy = s.key; sortOpen = false"
						>
							{{ store.getTranslation(s.i18nKey) }}
						</button>
					</div>
				</div>
			</div>

			<!-- Provider list -->
			<div class="max-h-[420px] overflow-y-auto">
				<div v-if="loading" class="flex items-center justify-center py-8 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<template v-else>
					<!-- Automatic -->
					<button
						class="w-full px-3 py-2.5 flex items-center gap-3 hover:bg-accent/50 transition-colors border-b border-border/50 text-left"
						@click="selectAuto"
					>
						<div class="h-7 w-7 rounded-lg bg-muted flex items-center justify-center shrink-0">
							<Sparkles class="h-3.5 w-3.5 text-muted-foreground" />
						</div>
						<div class="flex-1 min-w-0">
							<div class="text-sm font-medium">{{ store.getTranslation('chat.provider_selector.automatic') }}</div>
							<div class="text-xs text-muted-foreground">{{ store.getTranslation('chat.provider_selector.automatic_hint') }}</div>
						</div>
						<Check v-if="!chatStore.selectedProviderSlug" class="h-4 w-4 text-primary shrink-0" />
					</button>

					<div v-if="parentUnavailable" class="p-4 text-center text-xs text-muted-foreground">
						{{ store.getTranslation('chat.provider_selector.unavailable_for_key') }}
					</div>
					<template v-else-if="sortedOptions.length > 0">
						<!-- Column header -->
						<div class="grid grid-cols-[1fr_80px_56px_56px_40px_8px_20px] gap-2 px-3 py-1.5 text-[10px] uppercase tracking-wide text-muted-foreground border-b border-border/50 items-center">
							<span>{{ store.getTranslation('chat.provider_selector.col_provider') }}</span>
							<span class="text-right">{{ store.getTranslation('chat.provider_selector.col_price') }}</span>
							<span class="text-right">{{ store.getTranslation('chat.provider_selector.col_latency') }}</span>
							<span class="text-right">{{ store.getTranslation('chat.provider_selector.col_throughput') }}</span>
							<span class="text-right">{{ store.getTranslation('chat.provider_selector.col_context') }}</span>
							<span />
							<span />
						</div>

						<!-- Provider rows -->
						<div
							v-for="opt in sortedOptions"
							:key="opt.id"
							class="border-b border-border/50 last:border-0"
						>
							<div
								class="grid grid-cols-[1fr_80px_56px_56px_40px_8px_20px] gap-2 px-3 py-2.5 items-center cursor-pointer transition-colors"
								:class="[rowDisabled(opt) ? 'opacity-40 pointer-events-none' : 'hover:bg-accent/50', isSelected(opt) ? 'bg-primary/5' : '']"
								@click="selectOption(opt)"
							>
								<!-- Icon + name -->
								<div class="flex items-center gap-2 min-w-0">
									<div class="h-7 w-7 rounded-lg bg-muted flex items-center justify-center shrink-0 text-foreground overflow-hidden">
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
									<div class="min-w-0">
										<div class="text-sm font-medium truncate">{{ opt.provider_name || opt.provider_slug || '—' }}</div>
										<div class="text-[11px] text-muted-foreground truncate">{{ opt.provider_slug }}</div>
									</div>
								</div>

								<!-- Price -->
								<div class="text-xs font-medium tabular-nums text-right">
									{{ formatPrice(opt.price_input) }} / {{ formatPrice(opt.price_output) }}
								</div>

								<!-- Latency -->
								<div
									class="text-xs font-medium tabular-nums text-right"
									:class="opt.latency != null && opt.latency < 300 ? 'text-green-500' : ''"
								>
									{{ formatLatency(opt.latency) }}
								</div>

								<!-- Throughput -->
								<div class="text-xs font-medium tabular-nums text-right">
									{{ formatThroughput(opt.throughput) }}
								</div>

								<!-- Context -->
								<div class="text-xs font-medium tabular-nums text-right">
									{{ formatContext(opt.context_length) }}
								</div>

								<!-- Status dot -->
								<div class="h-2 w-2 rounded-full justify-self-center" :class="getStatusColor(opt)" />

								<!-- Expand -->
								<button
									class="text-muted-foreground hover:text-foreground transition-colors p-0.5 rounded justify-self-end"
									@click.stop="expandedId = expandedId === opt.id ? null : opt.id"
								>
									<ChevronDown class="h-3.5 w-3.5 transition-transform duration-150" :class="expandedId === opt.id ? 'rotate-180' : ''" />
								</button>
							</div>

							<!-- Expanded details -->
							<div v-if="expandedId === opt.id" class="px-4 py-2.5 bg-muted/30 border-t border-border/30 flex items-center gap-4 flex-wrap">
								<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
									<Lock class="h-3 w-3" />
									{{ store.getTranslation('chat.provider_selector.expanded_privacy') }}: {{ isPrivateProvider(opt) ? store.getTranslation('chat.provider_selector.privacy_on') : '—' }}
								</span>
								<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
									<div class="h-1.5 w-1.5 rounded-full" :class="opt.uptime != null && opt.uptime > 95 ? 'bg-green-500' : 'bg-yellow-500'" />
									{{ store.getTranslation('chat.provider_selector.expanded_uptime') }}: {{ formatUptime(opt.uptime) }}
								</span>
								<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
									<Gauge class="h-3 w-3" />
									{{ store.getTranslation('chat.provider_selector.expanded_rate_limits') }}: —
								</span>
								<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
									<Wrench class="h-3 w-3" />
									{{ store.getTranslation('chat.provider_selector.expanded_tools') }}: —
								</span>
								<div class="ml-auto flex items-center gap-2 text-muted-foreground">
									<Shield class="h-3.5 w-3.5" />
									<Info class="h-3.5 w-3.5" />
								</div>
							</div>
						</div>
					</template>
					<div v-else class="p-4 text-center text-xs text-muted-foreground">
						{{ store.getTranslation('chat.provider_selector.no_providers') }}
					</div>
				</template>
			</div>
		</PopoverContent>
	</Popover>
</template>

<script setup lang="ts">
import {Server, ChevronDown, Check, Loader2, Sparkles, Search, Lock, Gauge, Wrench, Shield, Info, Bot} from 'lucide-vue-next';
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

const PRIVATE_PROVIDER_SLUGS = ['anthropic', 'azure', 'bedrock', 'google', 'vertex'];

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
const sortBy = ref<'recommended' | 'latency' | 'price' | 'throughput'>('recommended');
const sortOpen = ref(false);
const expandedId = ref<string | null>(null);

const sortOptions = [
	{key: 'recommended' as const, i18nKey: 'chat.provider_selector.sort_recommended'},
	{key: 'latency' as const, i18nKey: 'chat.provider_selector.sort_latency'},
	{key: 'price' as const, i18nKey: 'chat.provider_selector.sort_price'},
	{key: 'throughput' as const, i18nKey: 'chat.provider_selector.sort_throughput'},
];

const currentSortLabel = computed(() => {
	const opt = sortOptions.find(s => s.key === sortBy.value);
	return opt ? store.getTranslation(opt.i18nKey) : store.getTranslation('chat.provider_selector.sort_recommended');
});

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

function isPrivateProvider(opt: ProviderOption): boolean {
	const slug = (opt.provider_slug ?? '').toLowerCase();
	return PRIVATE_PROVIDER_SLUGS.some(p => slug.includes(p));
}

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
	if (sortBy.value === 'recommended') {
		list.sort((a, b) => recommendScore(b) - recommendScore(a));
	} else if (sortBy.value === 'latency') {
		list.sort((a, b) => (a.latency ?? Infinity) - (b.latency ?? Infinity));
	} else if (sortBy.value === 'price') {
		list.sort((a, b) => (a.price_input ?? Infinity) - (b.price_input ?? Infinity));
	} else if (sortBy.value === 'throughput') {
		list.sort((a, b) => (b.throughput ?? 0) - (a.throughput ?? 0));
	}
	return list;
});

function rowDisabled(opt: ProviderOption): boolean {
	return opt.status != null && opt.status < 0;
}

function isSelected(opt: ProviderOption): boolean {
	return !!chatStore.selectedProviderSlug && (opt.provider_slug || '') === chatStore.selectedProviderSlug;
}

function getStatusColor(opt: ProviderOption): string {
	if (opt.status == null) return 'bg-muted-foreground/40';
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

function formatContext(value: number | null): string {
	if (value == null) return '—';
	if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(0)}M`;
	if (value >= 1_000) return `${(value / 1_000).toFixed(0)}K`;
	return String(value);
}

function formatUptime(value: number | null): string {
	if (value == null) return '—';
	return `${value.toFixed(2)}%`;
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
		sortOpen.value = false;
		expandedId.value = null;
	}
});
</script>
