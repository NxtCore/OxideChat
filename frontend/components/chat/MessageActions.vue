<template>
	<div class="message-actions flex items-center gap-0.5 rounded-lg border border-border/50 bg-popover/80 backdrop-blur-sm p-0.5 shadow-lg">
		<!-- Copy message content -->
		<ShadTooltip>
			<ShadTooltipTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="copyContent">
					<Check v-if="copied" class="h-3.5 w-3.5 text-primary" />
					<Copy v-else class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="top" :side-offset="8">
				<p class="text-xs">{{ copied ? 'Copied!' : 'Copy message' }}</p>
			</ShadTooltipContent>
		</ShadTooltip>

		<!-- Regenerate (only for last assistant message) -->
		<ShadTooltip v-if="canRegenerate">
			<ShadTooltipTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="regenerate">
					<RefreshCw class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="top" :side-offset="8">
				<p class="text-xs">Regenerate response</p>
			</ShadTooltipContent>
		</ShadTooltip>

		<!-- Message info popover -->
		<ShadPopover>
			<ShadPopoverTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50">
					<Info class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadPopoverTrigger>

			<ShadPopoverContent class="w-64" align="start">
				<div class="space-y-3">
					<h4 class="font-medium text-foreground text-sm">Message Details</h4>

					<!-- Model -->
					<div v-if="modelName" class="flex justify-between text-xs">
						<span class="text-muted-foreground">Model</span>
						<span class="text-foreground font-medium">{{ modelName }}</span>
					</div>

					<div class="h-px bg-border" />

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
	</div>
</template>

<script setup lang="ts">
import {Copy, Check, RefreshCw, Info} from 'lucide-vue-next';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';

const props = defineProps<{
	message: ChatMessage;
	canRegenerate?: boolean;
	modelName?: string;
}>();

const chatStore = useChatStore();
const copied = ref(false);

async function copyContent() {
	if (!props.message.content) return;
	await navigator.clipboard.writeText(props.message.content);
	copied.value = true;
	setTimeout(() => {
		copied.value = false;
	}, 2000);
}

function regenerate() {
	// TODO: Implement regeneration
	console.log('Regenerate message:', props.message.id);
}

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
