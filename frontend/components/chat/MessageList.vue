<template>
	<div ref="containerRef" class="flex-1 overflow-y-auto p-4">
		<div v-if="loading" class="mx-auto max-w-4xl space-y-4">
			<div v-for="i in 3" :key="i" class="flex gap-3">
				<div class="h-8 w-8 animate-pulse rounded-full bg-muted" />
				<div class="flex-1 space-y-2">
					<div class="h-4 w-1/3 animate-pulse rounded bg-muted" />
					<div class="h-16 animate-pulse rounded-xl bg-muted" />
				</div>
			</div>
		</div>
		<div v-else class="mx-auto max-w-4xl space-y-4">
			<TransitionGroup name="message">
				<MessageItem v-for="message in messages" :key="message.client_id ?? message.id" :message="message" :animation="store.preferences?.streaming_animation" />
			</TransitionGroup>
		</div>

		<Transition name="fade">
			<ShadButton
				v-if="showScrollButton"
				class="absolute bottom-36 left-1/2 -translate-x-1/2 rounded-full bg-primary/25 text-primary backdrop-blur-sm hover:bg-background transition-colors"
				@click="handleScrollButtonClick"
			>
				<ArrowDown class="h-4 w-4" />
			</ShadButton>
		</Transition>
	</div>
</template>

<script setup lang="ts">
import {ArrowDown} from 'lucide-vue-next';
import MessageItem from './MessageItem.vue';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores/index';

const props = defineProps<{
	messages: ChatMessage[];
	loading: boolean;
}>();

const chatStore = useChatStore();
const store = useMainStore();
const containerRef = ref<HTMLElement>();
const showScrollButton = ref(false);
const isAutoScrolling = ref(true);

function scrollToBottom(smooth = true) {
	const container = containerRef.value;
	if (!container) return;

	// Use requestAnimationFrame for smoother scrolling
	requestAnimationFrame(() => {
		container.scrollTo({
			top: container.scrollHeight,
			behavior: smooth ? 'smooth' : 'auto',
		});
	});
}

function checkScrollPosition() {
	const container = containerRef.value;
	if (!container) return;
	const threshold = 100;
	const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < threshold;
	showScrollButton.value = !isNearBottom;
	isAutoScrolling.value = isNearBottom;
}

function handleScrollButtonClick() {
	scrollToBottom(true);
	isAutoScrolling.value = true;
}

// Watch for message content changes (during streaming)
watch(
	() => props.messages.map(m => `${m.content || ''}${m.reasoning_content || ''}`).join(''),
	() => {
		if (isAutoScrolling.value && chatStore.isStreaming) {
			nextTick(() => scrollToBottom(true));
		}
	},
	{deep: true}
);

onMounted(() => {
	const container = containerRef.value;
	if (container) {
		container.addEventListener('scroll', checkScrollPosition, {passive: true});
		scrollToBottom(false);
	}
});

onUnmounted(() => {
	const container = containerRef.value;
	if (container) {
		container.removeEventListener('scroll', checkScrollPosition);
	}
});

// Expose scrollToBottom for external use
defineExpose({
	scrollToBottom,
});
</script>

<style scoped>
.message-enter-active,
.message-leave-active {
	transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
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

/* Ensure smooth scrolling container */
.flex-1.overflow-y-auto {
	scroll-behavior: smooth;
}
</style>
