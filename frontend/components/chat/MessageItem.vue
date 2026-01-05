<template>
	<div class="group flex gap-4 py-6" :class="isUser ? 'flex-row-reverse' : ''">
		<!-- Avatar -->
		<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full overflow-hidden bg-muted">
			<User v-if="isUser" class="h-4 w-4 text-muted-foreground" />
			<template v-else>
				<div v-if="providerIcon?.type === 'svg'" class="h-full w-full p-1.5 [&>svg]:h-full [&>svg]:w-full" v-html="providerIcon.icon" />
				<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" class="h-full w-full object-contain p-1.5" alt="Provider icon" />
				<Bot v-else class="h-4 w-4 text-muted-foreground" />
			</template>
		</div>

		<!-- Message content -->
		<div class="flex flex-1 flex-col gap-2" :class="isUser ? 'items-end' : 'items-start'">
			<!-- Reasoning toggle (for assistant messages with reasoning) -->
			<div
				v-if="!isUser && (message.reasoning_content || isStreamingReasoning)"
				class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none"
				@click="showReasoning = !showReasoning"
			>
				<Brain class="h-3.5 w-3.5 fill-current" />
				<span class="text-[10px] font-bold uppercase tracking-widest">Reasoning</span>
				<ChevronDown class="h-3 w-3 transition-transform" :class="showReasoning ? 'rotate-180' : ''" />
			</div>

			<!-- Reasoning content -->
			<Transition name="expand">
				<div v-if="showReasoning && (message.reasoning_content || isStreamingReasoning)" class="w-full max-w-3xl rounded-xl bg-muted/50 border p-4">
					<div class="prose prose-sm dark:prose-invert max-w-none opacity-80" v-html="renderedReasoning" />
					<!-- Typing indicator for reasoning -->
					<div v-if="isStreamingReasoning && !message.reasoning_content" class="flex items-center gap-1 py-1">
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.15s" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.3s" />
					</div>
				</div>
			</Transition>

			<!-- Main content -->
			<div
				v-if="message.content || !isStreaming"
				class="prose prose-sm md:prose-base dark:prose-invert max-w-3xl"
				:class="isUser ? 'rounded-xl bg-muted/50 px-4 py-2 text-foreground' : ''"
				v-html="renderedContent"
			/>

			<!-- Typing indicator (for streaming assistant message with no content yet) -->
			<div v-else-if="!isUser && isStreaming && !message.content" class="flex items-center gap-1 py-2">
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.15s" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.3s" />
			</div>

			<!-- Message info button -->
			<MessageInfo v-if="!isUser && !isStreaming" :message="message" class="mt-1 opacity-0 transition-opacity group-hover:opacity-100" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {User, Bot, Brain, ChevronDown} from 'lucide-vue-next';
import MessageInfo from './MessageInfo.vue';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useIconsStore} from '~/stores/icons';

const props = defineProps<{
	message: ChatMessage;
	animation?: string;
}>();

const chatStore = useChatStore();
const iconStore = useIconsStore();
const showReasoning = ref(false);

const isUser = computed(() => props.message.role === 'user');
const isStreaming = computed(() => props.message.id.startsWith('streaming-'));
const isStreamingReasoning = computed(() => isStreaming.value && chatStore.isStreaming && !props.message.content);

const model = computed(() => {
	if (isUser.value) return null;
	return chatStore.models.find(m => m.model_id === props.message.model_id);
});

const providerIcon = computed(() => {
	if (!model.value) return null;
	if (model.value.provider_icon_svg) {
		return {type: 'svg' as const, icon: model.value.provider_icon_svg};
	}
	return iconStore.getProviderIcon(model.value.provider_name);
});

// Auto-expand reasoning while streaming
watch(
	() => props.message.reasoning_content,
	newVal => {
		if (newVal && isStreaming.value && !props.message.content) {
			showReasoning.value = true;
		}
	},
	{immediate: true}
);

// Simple markdown rendering (in production, use a proper markdown library or remend)
const renderedContent = computed(() => {
	return renderMarkdown(props.message.content);
});

const renderedReasoning = computed(() => {
	return props.message.reasoning_content ? renderMarkdown(props.message.reasoning_content) : '';
});

function renderMarkdown(text: string): string {
	if (!text) return '';
	// Basic rendering - escape HTML and convert code blocks
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre class="bg-muted p-4 rounded-lg my-2 overflow-x-auto"><code class="language-$1">$2</code></pre>')
		.replace(/`([^`]+)`/g, '<code class="bg-muted px-1 rounded text-sm">$1</code>')
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
