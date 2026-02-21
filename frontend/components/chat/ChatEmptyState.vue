<template>
	<div class="flex flex-1 flex-col items-center justify-center p-8">
		<img src="/light_transparent.svg" alt="OxideChat Logo" class="h-20 w-20 mb-8" />
		<h1 class="mb-2 text-2xl font-semibold text-foreground">{{ greeting }}, {{ username }}</h1>
		<p class="mb-8 max-w-md text-center text-muted-foreground">
			{{ description }}
		</p>
		<div class="mb-8 w-full max-w-2xl">
			<ChatComposer @send="onSend" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {MessageSquare} from 'lucide-vue-next';
import ChatComposer from './ChatComposer.vue';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const emit = defineEmits<{
	send: [content: string, parts?: any[]];
}>();

const store = useMainStore();

const username = computed(() => store.auth.user?.username || store.getTranslation('chat.empty_state.username_default'));

const greeting = computed(() => {
	const hour = new Date().getHours();
	if (hour < 12) return store.getTranslation('chat.empty_state.greeting_morning');
	if (hour < 18) return store.getTranslation('chat.empty_state.greeting_afternoon');
	return store.getTranslation('chat.empty_state.greeting_evening');
});

const description = computed(() => {
	const placeholders = [
		store.getTranslation('chat.empty_state.desc_1'),
		store.getTranslation('chat.empty_state.desc_2'),
		store.getTranslation('chat.empty_state.desc_3'),
		store.getTranslation('chat.empty_state.desc_4'),
		store.getTranslation('chat.empty_state.desc_5'),
	];
	return placeholders[Math.floor(Math.random() * placeholders.length)];
});

function onSend(content: string, parts?: any[]) {
	if (!content && (!parts || parts.length === 0)) return;
	emit('send', content, parts);
}
</script>
