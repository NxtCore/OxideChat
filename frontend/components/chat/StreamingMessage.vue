<template>
	<div class="flex gap-3">
		<!-- Avatar -->
		<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
			<Bot class="h-4 w-4" />
		</div>

		<!-- Streaming content -->
		<div class="flex max-w-[80%] flex-col gap-1">
			<span class="text-xs text-muted-foreground">Assistant</span>

			<div class="relative rounded-2xl rounded-bl-md bg-muted px-4 py-3 text-foreground">
				<!-- Typing indicator or content -->
				<div v-if="!content" class="flex items-center gap-1">
					<span class="typing-dot" />
					<span class="typing-dot delay-150" />
					<span class="typing-dot delay-300" />
				</div>

				<!-- Content with animation -->
				<div v-else class="prose prose-sm dark:prose-invert max-w-none" :class="animationClass" v-html="renderedContent" />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Bot} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';

const props = defineProps<{
	animation?: 'fade' | 'typewriter' | 'slide' | 'none';
}>();

const chatStore = useChatStore();

// In a real implementation, this would be connected to SSE stream
const content = ref('');

const animationClass = computed(() => {
	switch (props.animation) {
		case 'typewriter':
			return 'animate-typewriter';
		case 'slide':
			return 'animate-slide-up';
		case 'fade':
			return 'animate-fade-in';
		default:
			return '';
	}
});

const renderedContent = computed(() => {
	return content.value
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code class="language-$1">$2</code></pre>')
		.replace(/`([^`]+)`/g, '<code>$1</code>')
		.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
		.replace(/\*([^*]+)\*/g, '<em>$1</em>')
		.replace(/\n/g, '<br>');
});
</script>

<style scoped>
.typing-dot {
	@apply h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50;
}

.delay-150 {
	animation-delay: 0.15s;
}

.delay-300 {
	animation-delay: 0.3s;
}

.animate-fade-in {
	animation: fadeIn 0.3s ease-out;
}

.animate-slide-up {
	animation: slideUp 0.3s ease-out;
}

.animate-typewriter {
	overflow: hidden;
	animation: typewriter 0.05s steps(1) infinite;
}

@keyframes fadeIn {
	from {
		opacity: 0;
	}
	to {
		opacity: 1;
	}
}

@keyframes slideUp {
	from {
		opacity: 0;
		transform: translateY(10px);
	}
	to {
		opacity: 1;
		transform: translateY(0);
	}
}
</style>
