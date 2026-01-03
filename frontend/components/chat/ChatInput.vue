<template>
	<div class="border-t border-border bg-background/80 p-4 backdrop-blur-sm">
		<div class="mx-auto max-w-4xl">
			<!-- Tool bar -->
			<div class="mb-3 flex items-center gap-2">
				<ModelSelector />
				<ReasoningSelector v-if="chatStore.hasReasoningCapability" />
				<ToolSelector />
				<div class="flex-1" />
				<ContextLimitIndicator />
			</div>

			<!-- Input area -->
			<div class="relative">
				<ShadTextarea
					ref="textareaRef"
					v-model="message"
					:placeholder="placeholder"
					rows="1"
					class="w-full resize-none rounded-xl border border-border bg-card px-4 py-3 pr-12 text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/50"
					:disabled="chatStore.isStreaming"
					@keydown.enter.exact="handleEnter"
					@input="autoResize"
				/>
				<ShadButton
					class="absolute bottom-3 right-3 rounded-lg bg-primary p-2 text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
					:disabled="!canSend"
					@click="sendMessage"
				>
					<Send v-if="!chatStore.isStreaming" class="h-4 w-4" />
					<Loader2 v-else class="h-4 w-4 animate-spin" />
				</ShadButton>
			</div>

			<!-- Hint text -->
			<p class="mt-2 text-center text-xs text-muted-foreground">Press Enter to send, Shift+Enter for new line</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Send, Loader2} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const emit = defineEmits<{
	send: [content: string];
}>();

const chatStore = useChatStore();
const mainStore = useMainStore();

const message = ref('');
const textareaRef = ref<HTMLTextAreaElement>();

const placeholder = computed(() => {
	if (chatStore.selectedModel) {
		return `Message ${chatStore.selectedModel.display_name}...`;
	}
	return 'Select a model and start typing...';
});

const canSend = computed(() => {
	return message.value.trim().length > 0 && chatStore.selectedModel && !chatStore.isStreaming;
});

function handleEnter(e: KeyboardEvent) {
	if (!e.shiftKey && canSend.value) {
		e.preventDefault();
		sendMessage();
	}
}

function sendMessage() {
	if (!canSend.value) return;
	emit('send', message.value.trim());
	message.value = '';
	nextTick(() => autoResize());
}

function autoResize() {
	const textarea = textareaRef.value;
	if (!textarea) return;
	textarea.style.height = 'auto';
	textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}
</script>
