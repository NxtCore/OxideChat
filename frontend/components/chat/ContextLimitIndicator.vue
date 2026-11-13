<template>
	<ShadTooltip>
		<ShadTooltipTrigger as-child>
			<button
				type="button"
				data-testid="context-indicator"
				class="inline-flex h-8 items-center gap-1 rounded-md px-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
				:aria-label="store.getTranslation('chat.context_indicator.percent_used', {percent: displayPercentage})"
			>
				<svg class="h-5 w-5 -rotate-90" viewBox="0 0 36 36" aria-hidden="true">
					<circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor" stroke-width="4" class="text-muted-foreground/20" />
					<circle
						cx="18"
						cy="18"
						r="15.5"
						fill="none"
						stroke="currentColor"
						stroke-width="4"
						:stroke-dasharray="`${circumference} ${circumference}`"
						:stroke-dashoffset="strokeOffset"
						stroke-linecap="round"
						class="transition-all duration-300"
						:class="colorClass"
					/>
				</svg>
				<span v-if="showNumber" class="text-[10px] font-medium tabular-nums" :class="colorClass" data-testid="context-indicator-percent">{{ displayPercentage }}%</span>
			</button>
		</ShadTooltipTrigger>

		<ShadTooltipContent side="bottom">
			<div class="space-y-1 text-xs">
				<p class="font-medium">{{ store.getTranslation('chat.context_indicator.usage') }}</p>
				<p>{{ store.getTranslation('chat.context_indicator.percent_used', {percent: displayPercentage}) }}</p>
				<p class="text-muted-foreground">{{ formatTokens(modelStore.contextTokens) }} / {{ formatTokens(contextLimit) }} {{ store.getTranslation('chat.context_indicator.tokens') }}</p>
				<p v-if="isNearLimit" class="text-amber-500">{{ store.getTranslation('chat.context_indicator.approaching_limit') }}</p>
			</div>
		</ShadTooltipContent>
	</ShadTooltip>
</template>

<script setup lang="ts">
import {useModelStore} from '~/stores/modelStore';
import {useMainStore} from '~/stores';

const modelStore = useModelStore();
const store = useMainStore();
const circumference = 2 * Math.PI * 15.5;

const contextLimit = computed(() => {
	return modelStore.selectedModel?.context_length || 128000;
});

const percentage = computed(() => {
	return modelStore.contextPercentage;
});

const displayPercentage = computed(() => {
	return Math.round(percentage.value);
});

const strokeOffset = computed(() => {
	return circumference - (percentage.value / 100) * circumference;
});

const showNumber = computed(() => percentage.value >= 50);

const isNearLimit = computed(() => {
	return percentage.value > 80;
});

const colorClass = computed(() => {
	if (percentage.value > 90) return 'text-destructive';
	if (percentage.value > 75) return 'text-amber-500';
	return 'text-primary';
});

function formatTokens(tokens: number): string {
	if (tokens >= 1000000) return `${(tokens / 1000000).toFixed(1)}M`;
	if (tokens >= 1000) return `${(tokens / 1000).toFixed(0)}K`;
	return tokens.toString();
}
</script>
