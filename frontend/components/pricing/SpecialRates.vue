<template>
	<ShadTooltip v-if="hasSpecialRates(summary)">
		<ShadTooltipTrigger as-child>
			<span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-1.5 py-0.5 text-primary" tabindex="0" :aria-label="lines.join('. ')" data-testid="pricing-special-rates">
				<Layers v-if="summary!.context_tiers.length > 0" class="h-3 w-3" />
				<Gauge v-if="summary!.service_tiers.length > 0" class="h-3 w-3" />
				<Clock v-if="summary!.windows > 0" class="h-3 w-3" />
			</span>
		</ShadTooltipTrigger>
		<ShadTooltipContent class="max-w-sm">
			<p class="mb-1 text-xs font-medium">{{ store.getTranslation('pricing.special_rates') }}</p>
			<p v-for="(line, index) in lines" :key="index" class="whitespace-pre text-xs">{{ line }}</p>
		</ShadTooltipContent>
	</ShadTooltip>
</template>

<script setup lang="ts">
import {Clock, Gauge, Layers} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {PriceSheet} from '~/types/generated/PriceSheet';
import type {PriceSummary} from '~/types/generated/PriceSummary';

const props = defineProps<{
	summary: PriceSummary | null | undefined;
	sheet?: PriceSheet | null;
}>();

const store = useMainStore();
const translate = (key: string, args?: Record<string, string | number>) => store.getTranslation(key, args);

const lines = computed(() => {
	if (!props.summary) return [];
	return props.sheet ? describeSheet(props.sheet, translate) : describeSummary(props.summary, translate);
});
</script>
