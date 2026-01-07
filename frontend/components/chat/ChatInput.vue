<template>
	<div class="p-4">
		<div class="mx-auto max-w-4xl">
			<div class="relative flex flex-col rounded-2xl border border-border bg-card shadow-sm">
				<ShadTextarea
					ref="textareaRef"
					v-model="message"
					:placeholder="placeholder"
					rows="1"
					class="min-h-15 w-full resize-none border-none bg-transparent px-4 py-3 text-foreground placeholder:text-muted-foreground focus-visible:ring-0"
					:disabled="chatStore.isStreaming"
					@keydown.enter.exact="handleEnter"
					@input="autoResize"
				/>

				<ChatComposer @send="sendMessage" />
			</div>

			<p class="mt-2 text-center text-[10px] text-muted-foreground opacity-50">{{ store.getTranslation('chat.composer.hint') }}</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import ChatComposer from './ChatComposer.vue';

const emit = defineEmits<{
	send: [content: string];
}>();

const chatStore = useChatStore();
const store = useMainStore();

const message = ref('');
const textareaRef = ref();

const placeholder = computed(() => {
	if (chatStore.selectedModel) {
		return store.getTranslation('chat.composer.placeholder_model', {model: chatStore.selectedModel.display_name});
	}
	return store.getTranslation('chat.composer.placeholder_default');
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
	if (!textareaRef.value) return;
	const textarea = textareaRef.value.$el instanceof HTMLTextAreaElement ? textareaRef.value.$el : textareaRef.value.$el?.querySelector('textarea');
	if (!textarea) return;
	textarea.style.height = 'auto';
	textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}
</script>
