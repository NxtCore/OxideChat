<template>
	<ShadTooltip>
		<ShadTooltipTrigger as-child>
			<div class="relative flex items-center justify-center">
				<!-- Donut chart -->
				<svg class="h-8 w-8 -rotate-90" viewBox="0 0 36 36">
					<!-- Background circle -->
					<circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor" stroke-width="3" class="text-muted/30" />
					<!-- Progress circle -->
					<circle
						cx="18"
						cy="18"
						r="15.5"
						fill="none"
						stroke="currentColor"
						stroke-width="3"
						:stroke-dasharray="`${circumference} ${circumference}`"
						:stroke-dashoffset="strokeOffset"
						stroke-linecap="round"
						class="transition-all duration-300"
						:class="colorClass"
					/>
				</svg>

				<!-- Center text -->
				<span class="absolute text-[8px] font-medium" :class="colorClass"> {{ displayPercentage }}% </span>
			</div>
		</ShadTooltipTrigger>

		<ShadTooltipContent side="bottom">
			<div class="space-y-1 text-xs">
				<p class="font-medium">Context Usage</p>
				<p class="text-muted-foreground">{{ formatTokens(chatStore.contextTokens) }} / {{ formatTokens(contextLimit) }} tokens</p>
				<p v-if="isNearLimit" class="text-amber-500">Approaching context limit</p>
			</div>
		</ShadTooltipContent>
	</ShadTooltip>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';

const chatStore = useChatStore();
const circumference = 2 * Math.PI * 15.5; // ~97.4

const contextLimit = computed(() => {
	return chatStore.selectedModel?.context_length || 128000;
});

const percentage = computed(() => {
	return chatStore.contextPercentage;
});

const displayPercentage = computed(() => {
	return Math.round(percentage.value);
});

const strokeOffset = computed(() => {
	return circumference - (percentage.value / 100) * circumference;
});

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
