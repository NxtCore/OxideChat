<template>
	<div class="rounded-lg border border-border bg-card p-4 space-y-5" data-testid="pricing-editor">
		<div class="flex items-start justify-between gap-3">
			<div>
				<h4 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.models.editor.pricing') }}</h4>
				<p class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.editor_desc') }}</p>
			</div>
			<ShadButton v-if="pricing?.override_pricing != null" variant="outline" size="sm" class="gap-1.5" data-testid="pricing-remove-override" @click="emit('removeOverride')">
				<Trash2 class="h-4 w-4" />
				{{ store.getTranslation('pricing.remove_override') }}
			</ShadButton>
		</div>

		<div class="grid gap-3 md:grid-cols-2">
			<div v-for="card in summaryCards" :key="card.key" class="rounded-md bg-muted/40 p-3 text-sm" :data-testid="`pricing-summary-${card.key}`">
				<p class="text-xs text-muted-foreground">{{ store.getTranslation(card.label) }}</p>
				<div v-if="card.summary" class="mt-1 flex flex-wrap items-center gap-2">
					<span class="font-medium tabular-nums">{{ formatRange(card.summary.input) }} / {{ formatRange(card.summary.output) }}</span>
					<PricingSpecialRates :summary="card.summary" :sheet="card.sheet" />
				</div>
				<p v-else class="mt-1 text-muted-foreground">{{ store.getTranslation('pricing.no_price') }}</p>
			</div>
		</div>

		<section class="space-y-2">
			<h5 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{{ store.getTranslation('pricing.base_rates') }}</h5>
			<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
				<div v-for="field in RATE_FIELDS" :key="field" class="space-y-1.5">
					<ShadLabel :for="`pricing-rate-${field}`">{{ store.getTranslation(`pricing.rate.${field}`) }}</ShadLabel>
					<ShadInput
						:id="`pricing-rate-${field}`"
						v-model="draft.rates[field]"
						type="number"
						min="0"
						step="any"
						:placeholder="catalogPlaceholder(catalog?.[field])"
						:data-testid="`pricing-${field}`"
					/>
				</div>
			</div>
			<p class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.base_rates_hint') }}</p>
		</section>

		<section class="space-y-2" data-testid="pricing-tiers">
			<div class="flex items-center justify-between gap-3">
				<div>
					<h5 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{{ store.getTranslation('pricing.context_tiers') }}</h5>
					<p class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.context_tiers_desc') }}</p>
				</div>
				<label class="flex shrink-0 items-center gap-2 text-xs">
					{{ store.getTranslation('pricing.customize') }}
					<ShadSwitch :model-value="draft.tiers !== null" data-testid="pricing-tiers-custom" @update:model-value="toggleTiers" />
				</label>
			</div>
			<template v-if="draft.tiers === null">
				<ul v-if="catalog?.tiers?.length" class="space-y-1 rounded-md bg-muted/40 p-3 text-xs text-muted-foreground">
					<li v-for="tier in catalog.tiers" :key="tier.min_context_tokens">{{ describeTier(tier, translate) }}</li>
				</ul>
				<p v-else class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.catalog_none') }}</p>
			</template>
			<template v-else>
				<div v-for="(tier, index) in draft.tiers" :key="index" class="grid grid-cols-2 items-end gap-2 sm:grid-cols-[1.4fr_1fr_1fr_1fr_auto]">
					<div class="col-span-2 space-y-1 sm:col-span-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('pricing.threshold') }}</ShadLabel>
						<ShadInput v-model="tier.min_context_tokens" type="number" min="1" step="1" :data-testid="`pricing-tier-threshold-${index}`" />
					</div>
					<SettingsModelsPricingRateInput v-for="field in TIER_FIELDS" :key="field" v-model="tier.rates[field]" :field="field" :testid="`pricing-tier-${field}-${index}`" />
					<ShadButton variant="ghost" size="icon" class="h-9 w-9" :aria-label="store.getTranslation('pricing.remove')" :data-testid="`pricing-tier-remove-${index}`" @click="draft.tiers.splice(index, 1)">
						<X class="h-4 w-4" />
					</ShadButton>
				</div>
				<ShadButton variant="outline" size="sm" class="gap-1.5" data-testid="pricing-tier-add" @click="addTier">
					<Plus class="h-3.5 w-3.5" />
					{{ store.getTranslation('pricing.add_tier') }}
				</ShadButton>
			</template>
		</section>

		<section class="space-y-2" data-testid="pricing-service-tiers">
			<div class="flex items-center justify-between gap-3">
				<div>
					<h5 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{{ store.getTranslation('pricing.service_tiers') }}</h5>
					<p class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.service_tiers_desc') }}</p>
				</div>
				<label class="flex shrink-0 items-center gap-2 text-xs">
					{{ store.getTranslation('pricing.customize') }}
					<ShadSwitch :model-value="draft.service_tiers !== null" data-testid="pricing-service-custom" @update:model-value="toggleServiceTiers" />
				</label>
			</div>
			<template v-if="draft.service_tiers === null">
				<ul v-if="catalogServiceLines.length" class="space-y-1 rounded-md bg-muted/40 p-3 text-xs text-muted-foreground">
					<li v-for="(line, index) in catalogServiceLines" :key="index" class="whitespace-pre">{{ line }}</li>
				</ul>
				<p v-else class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.catalog_none') }}</p>
			</template>
			<template v-else>
				<div v-for="(service, index) in draft.service_tiers" :key="index" class="grid grid-cols-2 items-end gap-2 sm:grid-cols-[1.4fr_1fr_1fr_1fr_auto]">
					<div class="col-span-2 space-y-1 sm:col-span-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('pricing.service_tier_name') }}</ShadLabel>
						<ShadInput v-model="service.name" list="pricing-service-tier-names" placeholder="flex" :data-testid="`pricing-service-name-${index}`" />
					</div>
					<SettingsModelsPricingRateInput v-for="field in TIER_FIELDS" :key="field" v-model="service.rates[field]" :field="field" :testid="`pricing-service-${field}-${index}`" />
					<ShadButton variant="ghost" size="icon" class="h-9 w-9" :aria-label="store.getTranslation('pricing.remove')" :data-testid="`pricing-service-remove-${index}`" @click="draft.service_tiers.splice(index, 1)">
						<X class="h-4 w-4" />
					</ShadButton>
				</div>
				<datalist id="pricing-service-tier-names">
					<option v-for="name in ['flex', 'priority', 'batch', 'scale']" :key="name" :value="name" />
				</datalist>
				<ShadButton variant="outline" size="sm" class="gap-1.5" data-testid="pricing-service-add" @click="addServiceTier">
					<Plus class="h-3.5 w-3.5" />
					{{ store.getTranslation('pricing.add_service_tier') }}
				</ShadButton>
			</template>
		</section>

		<section class="space-y-2" data-testid="pricing-windows">
			<div>
				<h5 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{{ store.getTranslation('pricing.windows') }}</h5>
				<p class="text-xs text-muted-foreground">{{ store.getTranslation('pricing.windows_desc') }}</p>
			</div>
			<div v-for="(window, index) in draft.windows ?? []" :key="index" class="space-y-2 rounded-md border border-border p-3">
				<div class="flex flex-wrap items-center gap-1" role="group" :aria-label="store.getTranslation('pricing.days')">
					<button
						v-for="day in WEEKDAYS"
						:key="day"
						type="button"
						class="rounded-md border px-2 py-1 text-xs transition-colors"
						:class="window.days.includes(day) ? 'border-primary bg-primary text-primary-foreground' : 'border-border text-muted-foreground hover:bg-muted'"
						:aria-pressed="window.days.includes(day)"
						:data-testid="`pricing-window-day-${day}-${index}`"
						@click="toggleDay(window.days, day)"
					>
						{{ store.getTranslation(`pricing.day_short.${day}`) }}
					</button>
					<span class="ml-1 text-xs text-muted-foreground">{{ window.days.length === 0 ? store.getTranslation('pricing.days_every') : '' }}</span>
				</div>
				<div class="grid grid-cols-2 items-end gap-2 sm:grid-cols-[1fr_1fr_1.4fr_1fr_1fr_auto]">
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('pricing.start') }}</ShadLabel>
						<ShadInput v-model="window.start" type="time" :data-testid="`pricing-window-start-${index}`" />
					</div>
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('pricing.end') }}</ShadLabel>
						<ShadInput v-model="window.end" type="time" :data-testid="`pricing-window-end-${index}`" />
					</div>
					<div class="col-span-2 space-y-1 sm:col-span-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('pricing.timezone') }}</ShadLabel>
						<ShadInput v-model="window.timezone" list="pricing-timezones" :data-testid="`pricing-window-timezone-${index}`" />
					</div>
					<SettingsModelsPricingRateInput v-for="field in WINDOW_FIELDS" :key="field" v-model="window.rates[field]" :field="field" :testid="`pricing-window-${field}-${index}`" />
					<ShadButton variant="ghost" size="icon" class="h-9 w-9" :aria-label="store.getTranslation('pricing.remove')" :data-testid="`pricing-window-remove-${index}`" @click="removeWindow(index)">
						<X class="h-4 w-4" />
					</ShadButton>
				</div>
			</div>
			<datalist id="pricing-timezones">
				<option v-for="zone in timezones" :key="zone" :value="zone" />
			</datalist>
			<ShadButton variant="outline" size="sm" class="gap-1.5" data-testid="pricing-window-add" @click="addWindow">
				<Plus class="h-3.5 w-3.5" />
				{{ store.getTranslation('pricing.add_window') }}
			</ShadButton>
		</section>

		<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.pricing_unit') }}</p>
	</div>
</template>

<script setup lang="ts">
import {Plus, Trash2, X} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {ModelPricing} from '~/types/generated/ModelPricing';
import type {PriceWeekday} from '~/types/generated/PriceWeekday';

const props = defineProps<{
	draft: PricingDraft;
	pricing: ModelPricing | null;
}>();

const emit = defineEmits<{removeOverride: []}>();

const store = useMainStore();
const translate = (key: string, args?: Record<string, string | number>) => store.getTranslation(key, args);

const TIER_FIELDS = ['input', 'output', 'cache_read'] as const;
const WINDOW_FIELDS = ['input', 'output'] as const;
const FALLBACK_TIMEZONES = ['UTC', 'Europe/Berlin', 'Europe/London', 'America/New_York', 'America/Los_Angeles', 'Asia/Shanghai', 'Asia/Tokyo'];

const catalog = computed(() => props.pricing?.catalog?.price ?? null);
const catalogServiceLines = computed(() => describeSheet({service_tiers: catalog.value?.service_tiers ?? {}}, translate));
const summaryCards = computed(() => [
	{key: 'catalog', label: 'pricing.catalog_price', summary: props.pricing?.catalog?.summary ?? null, sheet: catalog.value},
	{key: 'billed', label: 'pricing.billed_price', summary: props.pricing?.effective?.summary ?? null, sheet: props.pricing?.effective?.price ?? null},
]);
const timezones = computed(() => {
	const supported = (Intl as unknown as {supportedValuesOf?: (key: string) => string[]}).supportedValuesOf?.('timeZone');
	return supported?.length ? ['UTC', ...supported] : FALLBACK_TIMEZONES;
});

function catalogPlaceholder(value: number | undefined): string {
	return value == null ? store.getTranslation('pricing.not_set') : store.getTranslation('pricing.catalog_value', {rate: formatRate(value)});
}

function toggleTiers(custom: boolean) {
	props.draft.tiers = custom ? tiersToDraft(catalog.value?.tiers ?? []) : null;
}

function toggleServiceTiers(custom: boolean) {
	props.draft.service_tiers = custom ? serviceTiersToDraft(catalog.value?.service_tiers ?? {}) : null;
}

function addTier() {
	const last = props.draft.tiers?.at(-1);
	const next = last ? Number(last.min_context_tokens) * 2 : 200001;
	props.draft.tiers?.push({min_context_tokens: String(next || 200001), rates: emptyRateDraft()});
}

function addServiceTier() {
	props.draft.service_tiers?.push({name: '', rates: emptyRateDraft(), tiers: []});
}

function addWindow() {
	props.draft.windows ??= [];
	props.draft.windows.push({days: [], start: '00:00', end: '08:00', timezone: 'UTC', rates: emptyRateDraft()});
}

function removeWindow(index: number) {
	props.draft.windows?.splice(index, 1);
	if (props.draft.windows?.length === 0) props.draft.windows = null;
}

function toggleDay(days: PriceWeekday[], day: PriceWeekday) {
	const index = days.indexOf(day);
	if (index >= 0) days.splice(index, 1);
	else days.push(day);
	days.sort((a, b) => WEEKDAYS.indexOf(a) - WEEKDAYS.indexOf(b));
}
</script>
