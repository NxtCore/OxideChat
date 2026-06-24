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
				<Server class="h-4 w-4" />
				<span class="max-w-32 truncate font-medium">
					{{ selectedLabel }}
				</span>
				<ChevronDown class="h-3 w-3 text-muted-foreground" />
			</button>
		</PopoverTrigger>
		<PopoverContent class="w-140 p-0 overflow-hidden" align="start" :side-offset="8">
			<div class="flex items-center justify-between gap-2 p-2 border-b border-border">
				<span class="text-xs font-medium text-foreground px-1">{{ store.getTranslation('chat.provider_selector.title') }}</span>
				<div class="flex items-center rounded-md border border-border p-0.5">
					<button
						v-for="mode in modes"
						:key="mode"
						class="px-2 py-0.5 text-xs rounded transition-colors"
						:class="chatStore.providerRoutingMode === mode ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:bg-accent'"
						@click="setMode(mode)"
					>
						{{ store.getTranslation(`chat.provider_selector.mode_${mode}`) }}
					</button>
				</div>
			</div>

			<div class="max-h-100 overflow-y-auto">
				<div v-if="loading" class="flex items-center justify-center py-8 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<template v-else>
					<!-- Automatic (clears the pin) -->
					<button
						class="w-full px-3 py-2 flex items-center gap-2 hover:bg-accent/50 transition-colors border-b border-border/50 text-left"
						@click="selectAuto"
					>
						<Sparkles class="h-4 w-4 text-muted-foreground" />
						<div class="flex-1 min-w-0">
							<div class="text-sm font-medium">{{ store.getTranslation('chat.provider_selector.automatic') }}</div>
							<div class="text-xs text-muted-foreground">{{ store.getTranslation('chat.provider_selector.automatic_hint') }}</div>
						</div>
						<Check v-if="!chatStore.selectedProviderSlug" class="h-4 w-4 text-primary" />
					</button>

					<div v-if="parentUnavailable" class="p-4 text-center text-xs text-muted-foreground">
						{{ store.getTranslation('chat.provider_selector.unavailable_for_key') }}
					</div>
					<div v-else-if="options.length === 0" class="p-4 text-center text-xs text-muted-foreground">
						{{ store.getTranslation('chat.provider_selector.no_providers') }}
					</div>

					<!-- Header row -->
					<div v-if="options.length > 0" class="grid grid-cols-12 gap-2 px-3 py-1.5 text-[10px] uppercase tracking-wide text-muted-foreground border-b border-border/50">
						<span class="col-span-5">{{ store.getTranslation('chat.provider_selector.col_provider') }}</span>
						<span class="col-span-3 text-right">{{ store.getTranslation('chat.provider_selector.col_price') }}</span>
						<span class="col-span-2 text-right">{{ store.getTranslation('chat.provider_selector.col_latency') }}</span>
						<span class="col-span-2 text-right">{{ store.getTranslation('chat.provider_selector.col_throughput') }}</span>
					</div>

					<button
						v-for="opt in options"
						:key="opt.id"
						class="w-full grid grid-cols-12 gap-2 px-3 py-2 items-center hover:bg-accent/50 transition-colors border-b border-border/50 last:border-0 text-left"
						:class="rowDisabled(opt) ? 'opacity-50' : ''"
						@click="selectOption(opt)"
					>
						<div class="col-span-5 min-w-0 flex items-center gap-2">
							<Check v-if="isSelected(opt)" class="h-3.5 w-3.5 text-primary shrink-0" />
							<span v-else class="w-3.5 shrink-0" />
							<div class="min-w-0">
								<div class="text-sm font-medium truncate">{{ opt.provider_name || opt.provider_slug || '—' }}</div>
								<div class="text-[11px] text-muted-foreground truncate">
									{{ opt.quantization && opt.quantization !== 'unknown' ? opt.quantization : opt.provider_slug }}
								</div>
							</div>
						</div>
						<div class="col-span-3 text-right text-xs tabular-nums">{{ formatPrice(opt.price_input) }} / {{ formatPrice(opt.price_output) }}</div>
						<div class="col-span-2 text-right text-xs tabular-nums text-muted-foreground">{{ formatLatency(opt.latency) }}</div>
						<div class="col-span-2 text-right text-xs tabular-nums text-muted-foreground">{{ formatThroughput(opt.throughput) }}</div>
					</button>
				</template>
			</div>
		</PopoverContent>
	</Popover>
</template>

<script setup lang="ts">
import {Server, ChevronDown, Check, Loader2, Sparkles} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
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

const isOpen = ref(false);
const loading = ref(false);
const options = ref<ProviderOption[]>([]);
const parentUnavailable = ref(false);
const loadedModelId = ref<string | null>(null);

const modes = ['prefer', 'lock'] as const;

const isVisible = computed(() => {
	return (store.base?.enable_provider_selector ?? false) && chatStore.selectedModel?.provider?.kind === 'OPENROUTER';
});

const selectedLabel = computed(() => {
	if (!chatStore.selectedProviderSlug) return store.getTranslation('chat.provider_selector.automatic');
	const match = options.value.find(o => (o.provider_slug || '') === chatStore.selectedProviderSlug);
	return match?.provider_name || chatStore.selectedProviderSlug;
});

function rowDisabled(opt: ProviderOption): boolean {
	return opt.status != null && opt.status < 0;
}

function isSelected(opt: ProviderOption): boolean {
	return !!chatStore.selectedProviderSlug && (opt.provider_slug || '') === chatStore.selectedProviderSlug;
}

function formatPrice(value: number | null): string {
	if (value == null) return '—';
	return `$${value.toFixed(2)}`;
}

function formatLatency(value: number | null): string {
	if (value == null) return '—';
	// OpenRouter reports latency in milliseconds.
	return `${(value / 1000).toFixed(2)}s`;
}

function formatThroughput(value: number | null): string {
	if (value == null) return '—';
	return `${Math.round(value)} tps`;
}

function setMode(mode: 'prefer' | 'lock') {
	chatStore.providerRoutingMode = mode;
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
	if (open) loadOptions();
});
</script>
