<template>
	<div class="p-0">
		<div class="mx-auto max-w-4xl">
			<div class="relative flex flex-col rounded-2xl border border-border bg-card shadow-sm">
				<!-- Textarea -->
				<ShadTextarea
					ref="textareaRef"
					v-model="message"
					:placeholder="placeholder"
					rows="1"
					class="min-h-[60px] w-full resize-none border-none bg-transparent px-4 py-3 text-foreground placeholder:text-muted-foreground focus-visible:ring-0"
					:disabled="chatStore.isStreaming"
					@keydown.enter.exact="handleEnter"
					@input="autoResize"
				/>

				<!-- Bottom toolbar -->
				<div class="flex items-center justify-between px-2 py-1">
					<div class="flex items-center gap-1">
						<ModelSelector class="!border-none !bg-transparent !shadow-none hover:bg-muted/50" />
						<ReasoningSelector v-if="chatStore.hasReasoningCapability" class="!border-none !bg-transparent !shadow-none hover:bg-muted/50" />
						<ToolSelector v-if="chatStore.hasToolCapability" class="!border-none !bg-transparent !shadow-none hover:bg-muted/50" />
					</div>

					<div class="flex items-center gap-2">
						<ContextLimitIndicator />
						<ShadButton
							class="h-8 w-8 rounded-full bg-primary p-0 text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
							:disabled="!canSend"
							@click="sendMessage"
						>
							<ArrowUp v-if="!chatStore.isStreaming" class="h-4 w-4" />
							<Loader2 v-else class="h-4 w-4 animate-spin" />
						</ShadButton>
					</div>
				</div>
			</div>

			<!-- Hint text -->
			<p class="mt-2 text-center text-[10px] text-muted-foreground opacity-50">Press Enter to send, Shift+Enter for new line</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ArrowUp, Loader2, Paperclip, Globe} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import ModelSelector from './ModelSelector.vue';
import ReasoningSelector from './ReasoningSelector.vue';
import ContextLimitIndicator from './ContextLimitIndicator.vue';
import ToolSelector from './ToolSelector.vue';

const emit = defineEmits<{send: (content: string) => void}>();

const chatStore = useChatStore();
const message = ref('');
const textareaRef = ref<HTMLTextAreaElement | null>(null);

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
	const textarea = textareaRef.value instanceof HTMLTextAreaElement ? textareaRef.value : textareaRef.value?.$el?.querySelector('textarea');
	if (!textarea) return;
	textarea.style.height = 'auto';
	textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}
</script>
