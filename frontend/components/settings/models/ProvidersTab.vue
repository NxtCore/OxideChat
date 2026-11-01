<template>
	<div class="space-y-5">
		<div class="flex items-start justify-between gap-3">
			<div>
				<h3 class="text-base font-semibold text-foreground mb-1">
					{{ store.getTranslation('settings.models.editor.providers') }}
				</h3>
				<p class="text-xs text-muted-foreground">
					{{ store.getTranslation('settings.models.editor.providers_desc') }}
				</p>
			</div>
			<ShadButton variant="outline" size="sm" class="gap-1.5 flex-shrink-0" :disabled="providerOptionsLoading" @click="loadProviderOptions(true)">
				<Loader2 v-if="providerOptionsLoading" class="h-3.5 w-3.5 animate-spin" />
				<RotateCw v-else class="h-3.5 w-3.5" />
				{{ store.getTranslation('settings.models.editor.providers_refresh') }}
			</ShadButton>
		</div>

		<div v-if="providerOptionsLoading && !providerOptions" class="flex items-center justify-center py-10 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>

		<div class="rounded-lg border border-border bg-card p-4">
			<div class="mb-3 flex items-center justify-between gap-3">
				<div>
					<h4 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.models.editor.pricing') }}</h4>
					<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.pricing_desc') }}</p>
				</div>
				<ShadButton v-if="pricing?.override_pricing != null" variant="outline" size="sm" @click="deletePricingOverride">
					<Trash2 class="h-4 w-4" />
				</ShadButton>
			</div>
			<div class="grid gap-3 md:grid-cols-2">
				<div class="rounded-md bg-muted/40 p-3 text-sm">
					<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.reported_price') }}</p>
					<p class="mt-1 font-medium">{{ formatPricingPair(pricing?.reported_input, pricing?.reported_output) }}</p>
				</div>
				<div class="rounded-md bg-muted/40 p-3 text-sm">
					<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.effective_price') }}</p>
					<p class="mt-1 font-medium">{{ formatPricingPair(pricing?.effective_input, pricing?.effective_output) }}</p>
				</div>
				<div v-for="field in pricingFields" :key="field" class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation(`settings.models.editor.override_${field}`) }}</ShadLabel>
					<ShadInput v-model="pricingForm[field]" type="number" min="0" step="0.000001" />
				</div>
			</div>
			<p class="mt-2 text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.pricing_unit') }}</p>
			<div class="mt-3 flex justify-end">
				<ShadButton size="sm" @click="savePricingOverride">{{ store.getTranslation('common.save') }}</ShadButton>
			</div>
		</div>

		<div v-if="!providerOptionsLoading && providerParentUnavailable" class="rounded-lg border border-border bg-muted/30 px-4 py-3 text-sm text-muted-foreground">
			{{ store.getTranslation('settings.models.editor.providers_user_unavailable') }}
		</div>

		<div v-else-if="!providerOptionsLoading && (!providerOptions || providerOptions.options.length === 0)" class="flex items-center justify-center py-10 text-sm text-muted-foreground">
			{{ store.getTranslation('settings.models.editor.providers_empty') }}
		</div>

		<div v-else-if="providerOptions" class="overflow-x-auto rounded-lg border border-border">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-border bg-muted/40 text-left text-xs text-muted-foreground">
						<th v-for="col in columns" :key="col.key" class="px-3 py-2 font-medium" :class="col.right && 'text-right'">
							{{ store.getTranslation(`settings.models.editor.provider_col_${col.key}`) }}
						</th>
					</tr>
				</thead>
				<tbody>
					<tr v-for="opt in providerOptions.options" :key="opt.id" class="border-b border-border/60 last:border-0" :class="rowStateClass(opt)">
						<td class="px-3 py-2">
							<div class="font-medium text-foreground">{{ opt.provider_name || opt.provider_slug || '—' }}</div>
							<div v-if="opt.endpoint_name" class="text-xs text-muted-foreground">{{ opt.endpoint_name }}</div>
						</td>
						<td class="px-3 py-2">
							<span class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium" :class="rowStateBadgeClass(opt)">
								{{ rowStateLabel(opt) }}
							</span>
						</td>
						<td class="px-3 py-2 text-muted-foreground">{{ opt.quantization || '—' }}</td>
						<td class="px-3 py-2 text-right tabular-nums text-muted-foreground">
							{{ opt.context_length ? opt.context_length.toLocaleString() : '—' }}
						</td>
						<td class="px-3 py-2 text-right tabular-nums text-muted-foreground">{{ formatPrice(opt) }}</td>
						<td class="px-3 py-2 text-right tabular-nums text-muted-foreground">{{ opt.latency != null ? (opt.latency / 1000).toFixed(2) + 's' : '—' }}</td>
						<td class="px-3 py-2 text-right tabular-nums text-muted-foreground">{{ opt.throughput != null ? Math.round(opt.throughput) + ' tps' : '—' }}</td>
						<td class="px-3 py-2 text-right tabular-nums text-muted-foreground">{{ opt.uptime != null ? opt.uptime.toFixed(1) + '%' : '—' }}</td>
					</tr>
				</tbody>
			</table>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Loader2, RotateCw, Trash2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{
	modelId: string;
	active: boolean;
}>();

const store = useMainStore();

const providerOptions = ref<Record<string, any> | null>(null);
const providerOptionsLoading = ref(false);
const providerOptionsLoaded = ref(false);
const providerParentUnavailable = computed(() => providerOptions.value?.availability_state === 'USER_UNAVAILABLE');
const pricing = ref<Record<string, any> | null>(null);
const pricingFields = ['input', 'output', 'reasoning', 'cache_read', 'cache_write'] as const;
const pricingForm = reactive({
	input: '0',
	output: '0',
	reasoning: '',
	cache_read: '',
	cache_write: '',
});

const columns = [
	{key: 'provider'},
	{key: 'status'},
	{key: 'quant'},
	{key: 'context', right: true},
	{key: 'price', right: true},
	{key: 'latency', right: true},
	{key: 'throughput', right: true},
	{key: 'uptime', right: true},
];

type ProviderRowState = 'user_unavailable' | 'unavailable' | 'available' | 'unknown';

function rowState(opt: Record<string, any>): ProviderRowState {
	if (providerParentUnavailable.value) return 'user_unavailable';
	if (opt.status === null || opt.status === undefined) return 'unknown';
	if (opt.status < 0) return 'unavailable';
	return 'available';
}

function rowStateClass(opt: Record<string, any>): string {
	const state = rowState(opt);
	if (state === 'user_unavailable') return 'opacity-50';
	if (state === 'unavailable' || state === 'unknown') return 'text-muted-foreground';
	return '';
}

function rowStateBadgeClass(opt: Record<string, any>): string {
	const state = rowState(opt);
	if (state === 'available') return 'bg-green-500/10 text-green-600';
	if (state === 'unavailable') return 'bg-red-500/10 text-red-600';
	return 'bg-muted text-muted-foreground';
}

function rowStateLabel(opt: Record<string, any>): string {
	return store.getTranslation(`settings.models.editor.row_${rowState(opt)}`);
}

function formatPrice(opt: Record<string, any>): string {
	if (opt.price_input == null && opt.price_output == null) return '—';
	const fmt = (v: number | null | undefined) => (v == null ? '—' : '$' + Number(v).toFixed(2));
	return `${fmt(opt.price_input)} / ${fmt(opt.price_output)}`;
}

function formatPricingPair(input: string | number | null | undefined, output: string | number | null | undefined): string {
	const fmt = (value: string | number | null | undefined) => (value == null ? '-' : '$' + Number(value).toFixed(6));
	return `${fmt(input)} / ${fmt(output)}`;
}

async function loadPricing() {
	try {
		pricing.value = await modelPricing(props.modelId);
		const override = pricing.value?.override_pricing ?? null;
		pricingForm.input = String(override?.input ?? pricing.value?.effective_input ?? 0);
		pricingForm.output = String(override?.output ?? pricing.value?.effective_output ?? 0);
		pricingForm.reasoning = override?.reasoning != null ? String(override.reasoning) : '';
		pricingForm.cache_read = override?.cache_read != null ? String(override.cache_read) : '';
		pricingForm.cache_write = override?.cache_write != null ? String(override.cache_write) : '';
	} catch (e) {
		console.error(e);
	}
}

async function savePricingOverride() {
	const optional = (value: string): number | undefined => {
		const trimmed = value.trim();
		return trimmed === '' ? undefined : Number(trimmed);
	};
	const isValidPrice = (v: number | undefined): boolean => v === undefined || (Number.isFinite(v) && v >= 0);
	const input = Number(pricingForm.input || 0);
	const output = Number(pricingForm.output || 0);
	const reasoning = optional(pricingForm.reasoning);
	const cacheRead = optional(pricingForm.cache_read);
	const cacheWrite = optional(pricingForm.cache_write);
	if (![input, output, reasoning, cacheRead, cacheWrite].every(isValidPrice)) {
		store.toast(store.getTranslation('common.error'), {type: 'error'});
		return;
	}
	pricing.value = await saveModelPricing(props.modelId, {
		pricing: {input, output, reasoning, cache_read: cacheRead, cache_write: cacheWrite, tiers: []},
	});
	store.toast(store.getTranslation('settings.models.editor.pricing_saved'), {type: 'success'});
}

async function deletePricingOverride() {
	await deleteModelPricing(props.modelId);
	await loadPricing();
	store.toast(store.getTranslation('settings.models.editor.pricing_deleted'), {type: 'success'});
}

async function loadProviderOptions(force: boolean) {
	providerOptionsLoading.value = true;
	try {
		providerOptions.value = await adminProviderOptions(props.modelId, force);
		providerOptionsLoaded.value = true;
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.editor.providers_load_error'), {type: 'error'});
	} finally {
		providerOptionsLoading.value = false;
	}
}

watch(
	() => props.active,
	active => {
		if (active && !providerOptionsLoaded.value) loadProviderOptions(false);
	},
);

onMounted(loadPricing);
</script>
