<template>
	<ShadTooltip>
		<ShadTooltipTrigger as-child>
			<div class="relative flex items-center justify-center">
				<svg class="h-8 w-8 -rotate-90" viewBox="0 0 36 36">
					<circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor" stroke-width="3" class="text-muted/30" />
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

				<span class="absolute text-[8px] font-medium" :class="colorClass"> {{ displayPercentage }}% </span>
			</div>
		</ShadTooltipTrigger>

		<ShadTooltipContent side="bottom">
			<div class="space-y-1 text-xs">
				<p class="font-medium">{{ store.getTranslation('chat.context_indicator.usage') }}</p>
				<p class="text-muted-foreground">{{ formatTokens(chatStore.contextTokens) }} / {{ formatTokens(contextLimit) }} {{ store.getTranslation('chat.context_indicator.tokens') }}</p>
				<p v-if="isNearLimit" class="text-amber-500">{{ store.getTranslation('chat.context_indicator.approaching_limit') }}</p>
			</div>
		</ShadTooltipContent>
	</ShadTooltip>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const chatStore = useChatStore();
const store = useMainStore();
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
