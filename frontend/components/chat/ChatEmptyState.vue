<template>
	<div class="flex flex-1 flex-col items-center justify-center p-8">
		<div class="mb-8 flex h-20 w-20 items-center justify-center rounded-2xl bg-linear-to-br from-primary/20 to-primary/5 shadow-lg">
			<MessageSquare class="h-10 w-10 text-primary" />
		</div>
		<h1 class="mb-2 text-2xl font-semibold text-foreground">{{ greeting }}, {{ username }}</h1>
		<p class="mb-8 max-w-md text-center text-muted-foreground">
			{{ description }}
		</p>
		<div class="mb-8 w-full max-w-2xl">
			<ChatComposer @send="onSend($event)" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {MessageSquare} from 'lucide-vue-next';
import ChatComposer from './ChatComposer.vue';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const emit = defineEmits<{
	send: [content: string];
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

function handleSend(content: string) {
	emit('send', content);
}

function onSend(content: string | undefined) {
	if (!content) return;
	handleSend(content);
}
</script>
