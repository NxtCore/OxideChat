<template>
	<div class="group flex gap-3" :class="isUser ? 'flex-row-reverse' : ''">
		<!-- Avatar -->
		<div
			class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
			:class="isUser ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'"
		>
			<User v-if="isUser" class="h-4 w-4" />
			<Bot v-else class="h-4 w-4" />
		</div>

		<!-- Message content -->
		<div class="flex max-w-[80%] flex-col gap-1" :class="isUser ? 'items-end' : 'items-start'">
			<!-- Role label -->
			<span class="text-xs text-muted-foreground">
				{{ isUser ? 'You' : 'Assistant' }}
			</span>

			<!-- Message bubble -->
			<div class="relative rounded-2xl px-4 py-3" :class="[isUser ? 'bg-primary text-primary-foreground rounded-br-md' : 'bg-muted text-foreground rounded-bl-md']">
				<!-- Reasoning toggle (for assistant messages with reasoning) -->
				<ShadButton
					v-if="!isUser && message.reasoning_content"
					class="mb-2 flex items-center gap-1 text-xs opacity-70 transition-opacity hover:opacity-100"
					@click="showReasoning = !showReasoning"
				>
					<Brain class="h-3 w-3" />
					<span>{{ showReasoning ? 'Hide' : 'Show' }} reasoning</span>
					<ChevronDown class="h-3 w-3 transition-transform" :class="showReasoning ? 'rotate-180' : ''" />
				</ShadButton>

				<!-- Reasoning content -->
				<Transition name="expand">
					<div v-if="showReasoning && message.reasoning_content" class="mb-3 rounded-lg bg-background/50 p-3 text-sm opacity-80">
						<div class="prose prose-sm dark:prose-invert max-w-none" v-html="renderedReasoning" />
					</div>
				</Transition>

				<!-- Main content -->
				<div class="prose prose-sm dark:prose-invert max-w-none" :class="isUser ? 'prose-invert' : ''" v-html="renderedContent" />
			</div>

			<!-- Message info button -->
			<MessageInfo v-if="!isUser" :message="message" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {User, Bot, Brain, ChevronDown} from 'lucide-vue-next';
import MessageInfo from './MessageInfo.vue';
import type {ChatMessage} from '~/types/chat';

const props = defineProps<{
	message: ChatMessage;
	animation?: string;
}>();

const showReasoning = ref(false);

const isUser = computed(() => props.message.role === 'user');

// Simple markdown rendering (in production, use a proper markdown library or remend)
const renderedContent = computed(() => {
	return renderMarkdown(props.message.content);
});

const renderedReasoning = computed(() => {
	return props.message.reasoning_content ? renderMarkdown(props.message.reasoning_content) : '';
});

function renderMarkdown(text: string): string {
	// Basic rendering - escape HTML and convert code blocks
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code class="language-$1">$2</code></pre>')
		.replace(/`([^`]+)`/g, '<code>$1</code>')
		.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
		.replace(/\*([^*]+)\*/g, '<em>$1</em>')
		.replace(/\n/g, '<br>');
}
</script>

<style scoped>
.expand-enter-active,
.expand-leave-active {
	transition: all 0.3s ease;
	overflow: hidden;
}

.expand-enter-from,
.expand-leave-to {
	opacity: 0;
	max-height: 0;
	padding-top: 0;
	padding-bottom: 0;
	margin-bottom: 0;
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}
</style>
