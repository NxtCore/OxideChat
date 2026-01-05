<template>
	<div class="flex flex-1 flex-col items-center justify-center p-8">
		<div class="mb-8 flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-primary/20 to-primary/5 shadow-lg">
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
import {MessageSquare, Send, Code, Lightbulb, FileText, Zap} from 'lucide-vue-next';
import ChatComposer from './ChatComposer.vue';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const emit = defineEmits<{
	send: [content: string];
}>();

const chatStore = useChatStore();
const mainStore = useMainStore();

const username = computed(() => mainStore.auth.user?.username || 'there');

const greeting = computed(() => {
	const hour = new Date().getHours();
	if (hour < 12) return 'Good morning';
	if (hour < 18) return 'Good afternoon';
	return 'Good evening';
});

const description = computed(() => {
	const placeholders = [
		'Ask me anything about code, math, or creative writing.',
		'I can help you brainstorm, analyze data, or write content.',
		"Need help with a project? Just describe what you're working on.",
		"I'm here to assist with research, explanations, or problem-solving.",
		"Start a conversation and let's explore ideas together.",
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

function selectHint(hint: {title: string; description: string; prompt?: string}) {
	if (!chatStore.selectedModel) return; // Don't send without a selected model
	const prompt = hint.prompt || `Help me ${hint.title.toLowerCase()}`;
	emit('send', prompt);
}

const hints = [
	{
		icon: Code,
		title: 'Write code',
		description: 'Help me build a function or debug an issue',
		prompt: 'Help me write a function that',
	},
	{
		icon: Lightbulb,
		title: 'Brainstorm ideas',
		description: 'Generate creative solutions to problems',
		prompt: 'Help me brainstorm ideas for',
	},
	{
		icon: FileText,
		title: 'Summarize content',
		description: 'Condense long documents or articles',
		prompt: 'Summarize the following:',
	},
	{
		icon: Zap,
		title: 'Quick answer',
		description: 'Get fast answers to any question',
		prompt: 'What is',
	},
];
</script>
