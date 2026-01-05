<template>
	<ShadPopover>
		<ShadPopoverTrigger as-child>
			<ShadButton
				variant="ghost"
				size="icon"
				class="h-5 w-5 rounded bg-primary p-0 text-primary-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:bg-primary/90"
			>
				<Info class="h-3 w-3" />
			</ShadButton>
		</ShadPopoverTrigger>

		<ShadPopoverContent class="w-64">
			<div class="space-y-3">
				<h4 class="font-medium text-foreground">Message Details</h4>

				<!-- Tokens -->
				<div class="space-y-1">
					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">Input tokens</span>
						<span class="text-foreground">{{ message.input_tokens?.toLocaleString() || '-' }}</span>
					</div>
					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">Output tokens</span>
						<span class="text-foreground">{{ message.output_tokens?.toLocaleString() || '-' }}</span>
					</div>
					<div v-if="message.reasoning_tokens" class="flex justify-between text-xs">
						<span class="text-muted-foreground">Reasoning tokens</span>
						<span class="text-foreground">{{ message.reasoning_tokens?.toLocaleString() }}</span>
					</div>
				</div>

				<div class="h-px bg-border" />

				<!-- Costs -->
				<div class="space-y-1">
					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">Input cost</span>
						<span class="text-foreground">{{ formatCost(message.input_cost_usd) }}</span>
					</div>
					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">Output cost</span>
						<span class="text-foreground">{{ formatCost(message.output_cost_usd) }}</span>
					</div>
					<div v-if="message.reasoning_cost_usd" class="flex justify-between text-xs">
						<span class="text-muted-foreground">Reasoning cost</span>
						<span class="text-foreground">{{ formatCost(message.reasoning_cost_usd) }}</span>
					</div>
					<div class="flex justify-between text-xs font-medium">
						<span class="text-muted-foreground">Total cost</span>
						<span class="text-primary">{{ formatCost(message.total_cost_usd) }}</span>
					</div>
				</div>

				<div class="h-px bg-border" />

				<!-- Latency -->
				<div class="space-y-1">
					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">Response latency</span>
						<span class="text-foreground">{{ formatLatency(message.latency_ms) }}</span>
					</div>
					<div v-if="message.reasoning_latency_ms" class="flex justify-between text-xs">
						<span class="text-muted-foreground">Reasoning latency</span>
						<span class="text-foreground">{{ formatLatency(message.reasoning_latency_ms) }}</span>
					</div>
				</div>

				<!-- Timestamp -->
				<div class="flex justify-between text-xs">
					<span class="text-muted-foreground">Created</span>
					<span class="text-foreground">{{ formatTime(message.created_at) }}</span>
				</div>
			</div>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {Info} from 'lucide-vue-next';
import type {ChatMessage} from '~/types/chat';

const props = defineProps<{
	message: ChatMessage;
}>();

const formattedCost = computed(() => {
	const cost = props.message.total_cost_usd;
	if (!cost) return '';
	const value = parseFloat(cost);
	if (value < 0.01) return `$${value.toFixed(4)}`;
	return `$${value.toFixed(2)}`;
});

function formatCost(cost: string | null): string {
	if (!cost) return '-';
	const value = parseFloat(cost);
	if (value === 0) return '$0';
	if (value < 0.0001) return `$${value.toFixed(6)}`;
	if (value < 0.01) return `$${value.toFixed(4)}`;
	return `$${value.toFixed(2)}`;
}

function formatLatency(ms: number | null): string {
	if (!ms) return '-';
	if (ms < 1000) return `${ms}ms`;
	return `${(ms / 1000).toFixed(2)}s`;
}

function formatTime(timestamp: string): string {
	return new Date(timestamp).toLocaleString();
}
</script>
