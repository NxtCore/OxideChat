<template>
	<div ref="containerRef" class="flex-1 overflow-y-auto p-4">
		<!-- Loading skeleton -->
		<div v-if="loading" class="space-y-4">
			<div v-for="i in 3" :key="i" class="flex gap-3">
				<div class="h-8 w-8 animate-pulse rounded-full bg-muted" />
				<div class="flex-1 space-y-2">
					<div class="h-4 w-1/3 animate-pulse rounded bg-muted" />
					<div class="h-16 animate-pulse rounded-xl bg-muted" />
				</div>
			</div>
		</div>

		<!-- Messages -->
		<div v-else class="mx-auto max-w-4xl space-y-4">
			<TransitionGroup name="message">
				<MessageItem v-for="message in messages" :key="message.id" :message="message" :animation="chatStore.preferences.streaming_animation" />
			</TransitionGroup>

			<!-- Streaming message -->
			<StreamingMessage v-if="chatStore.isStreaming" :animation="chatStore.preferences.streaming_animation" />
		</div>

		<!-- Scroll to bottom button -->
		<Transition name="fade">
			<ShadButton
				v-if="showScrollButton"
				class="fixed bottom-24 right-8 rounded-full bg-primary p-3 text-primary-foreground shadow-lg transition-all hover:bg-primary/90"
				@click="scrollToBottom"
			>
				<ArrowDown class="h-4 w-4" />
			</ShadButton>
		</Transition>
	</div>
</template>

<script setup lang="ts">
import {ArrowDown} from 'lucide-vue-next';
import MessageItem from './MessageItem.vue';
import StreamingMessage from './StreamingMessage.vue';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';

const props = defineProps<{
	messages: ChatMessage[];
	loading: boolean;
}>();

const chatStore = useChatStore();
const containerRef = ref<HTMLElement>();
const showScrollButton = ref(false);

function scrollToBottom(smooth = true) {
	const container = containerRef.value;
	if (!container) return;
	container.scrollTo({
		top: container.scrollHeight,
		behavior: smooth ? 'smooth' : 'auto',
	});
}

function checkScrollPosition() {
	const container = containerRef.value;
	if (!container) return;
	const threshold = 100;
	const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < threshold;
	showScrollButton.value = !isNearBottom;
}

// Auto-scroll when new messages arrive
watch(
	() => props.messages.length,
	() => {
		if (!showScrollButton.value) {
			nextTick(() => scrollToBottom(false));
		}
	}
);

onMounted(() => {
	const container = containerRef.value;
	if (container) {
		container.addEventListener('scroll', checkScrollPosition);
		scrollToBottom(false);
	}
});

onUnmounted(() => {
	const container = containerRef.value;
	if (container) {
		container.removeEventListener('scroll', checkScrollPosition);
	}
});
</script>

<style scoped>
.message-enter-active,
.message-leave-active {
	transition: all 0.3s ease;
}

.message-enter-from {
	opacity: 0;
	transform: translateY(20px);
}

.message-leave-to {
	opacity: 0;
	transform: translateX(-20px);
}

.fade-enter-active,
.fade-leave-active {
	transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
	opacity: 0;
}
</style>
