<template>
	<div class="message-actions flex items-center gap-0.5 rounded-lg border border-border/50 bg-popover/80 backdrop-blur-sm p-0.5 shadow-lg">
		<!-- Fork Navigator -->
		<template v-if="siblingCount > 1">
			<ShadButton
				variant="ghost"
				size="icon"
				class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50"
				:disabled="currentIndex <= 1"
				@click="$emit('navigate', 'prev')"
			>
				<ChevronLeft class="h-3.5 w-3.5" />
			</ShadButton>
			<span class="min-w-[2.5rem] text-center text-xs tabular-nums text-muted-foreground select-none">{{ currentIndex }}/{{ siblingCount }}</span>
			<ShadButton
				variant="ghost"
				size="icon"
				class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50"
				:disabled="currentIndex >= siblingCount"
				@click="$emit('navigate', 'next')"
			>
				<ChevronRight class="h-3.5 w-3.5" />
			</ShadButton>
			<div class="w-px h-4 bg-border/50 mx-0.5" />
		</template>

		<ShadTooltip>
			<ShadTooltipTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="copyContent">
					<Check v-if="copied" class="h-3.5 w-3.5 text-primary" />
					<Copy v-else class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="top" :side-offset="8">
				<p class="text-xs">{{ copied ? store.getTranslation('chat.message_actions.copied') : store.getTranslation('chat.message_actions.copy_message') }}</p>
			</ShadTooltipContent>
		</ShadTooltip>

		<ShadTooltip v-if="canRegenerate">
			<ShadTooltipTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="regenerate">
					<RefreshCw class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="top" :side-offset="8">
				<p class="text-xs">{{ store.getTranslation('chat.message_actions.regenerate') }}</p>
			</ShadTooltipContent>
		</ShadTooltip>

		<ShadTooltip>
			<ShadTooltipTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="handleBranch">
					<GitBranch class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="top" :side-offset="8">
				<p class="text-xs">Branch to new chat</p>
			</ShadTooltipContent>
		</ShadTooltip>

		<ShadPopover>
			<ShadPopoverTrigger as-child>
				<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50">
					<Info class="h-3.5 w-3.5" />
				</ShadButton>
			</ShadPopoverTrigger>

			<ShadPopoverContent class="w-64" align="start">
				<div class="space-y-3">
					<h4 class="font-medium text-foreground text-sm">{{ store.getTranslation('chat.message_actions.details') }}</h4>

					<div v-if="modelName" class="flex justify-between text-xs">
						<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.model') }}</span>
						<span class="text-foreground font-medium">{{ modelName }}</span>
					</div>

					<div class="h-px bg-border" />

					<div class="space-y-1">
						<div class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.input_tokens') }}</span>
							<span class="text-foreground">{{ message.input_tokens?.toLocaleString() || '-' }}</span>
						</div>
						<div class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.output_tokens') }}</span>
							<span class="text-foreground">{{ message.output_tokens?.toLocaleString() || '-' }}</span>
						</div>
						<div v-if="message.reasoning_tokens" class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.reasoning_tokens') }}</span>
							<span class="text-foreground">{{ message.reasoning_tokens?.toLocaleString() }}</span>
						</div>
					</div>

					<div class="h-px bg-border" />

					<div class="space-y-1">
						<div class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.input_cost') }}</span>
							<span class="text-foreground">{{ formatCost(message.input_cost_usd) }}</span>
						</div>
						<div class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.output_cost') }}</span>
							<span class="text-foreground">{{ formatCost(message.output_cost_usd) }}</span>
						</div>
						<div v-if="message.reasoning_cost_usd" class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.reasoning_cost') }}</span>
							<span class="text-foreground">{{ formatCost(message.reasoning_cost_usd) }}</span>
						</div>
						<div class="flex justify-between text-xs font-medium">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.total_cost') }}</span>
							<span class="text-primary">{{ formatCost(message.total_cost_usd) }}</span>
						</div>
					</div>

					<div class="h-px bg-border" />

					<div class="space-y-1">
						<div class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.response_latency') }}</span>
							<span class="text-foreground">{{ formatLatency(message.latency_ms) }}</span>
						</div>
						<div v-if="message.reasoning_latency_ms" class="flex justify-between text-xs">
							<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.reasoning_latency') }}</span>
							<span class="text-foreground">{{ formatLatency(message.reasoning_latency_ms) }}</span>
						</div>
					</div>

					<div class="flex justify-between text-xs">
						<span class="text-muted-foreground">{{ store.getTranslation('chat.message_actions.created') }}</span>
						<span class="text-foreground">{{ formatTime(message.created_at) }}</span>
					</div>
				</div>
			</ShadPopoverContent>
		</ShadPopover>
	</div>
</template>

<script setup lang="ts">
import {Copy, Check, RefreshCw, Info, ChevronLeft, ChevronRight, GitBranch} from 'lucide-vue-next';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const store = useMainStore();

const props = defineProps<{
	message: ChatMessage;
	canRegenerate?: boolean;
	modelName?: string;
	currentIndex?: number;
	siblingCount?: number;
}>();

defineEmits<{
	navigate: [direction: 'prev' | 'next'];
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

async function handleBranch() {
	if (!chatStore.activeChat) return;
	await chatStore.branchFromMessage(chatStore.activeChat.id, props.message.id);
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
