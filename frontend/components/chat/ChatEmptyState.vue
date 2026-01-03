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
			<div class="mb-3 flex items-center gap-2">
				<ModelSelector />
				<ReasoningSelector v-if="chatStore.hasReasoningCapability" />
				<ToolSelector />
				<div class="flex-1" />
				<ContextLimitIndicator />
			</div>

			<div class="relative">
				<ShadTextarea
					ref="inputRef"
					v-model="inputMessage"
					placeholder="Ask me anything..."
					rows="1"
					class="w-full resize-none rounded-xl border border-border bg-card px-4 py-3 pr-12 text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/50"
					@keydown.enter.exact="handleSend"
					@input="autoResize"
				/>
				<ShadButton
					class="absolute bottom-3 right-3 rounded-lg bg-primary p-2 text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
					:disabled="!canSend"
					@click="handleSend"
				>
					<Send class="h-4 w-4" />
				</ShadButton>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {MessageSquare, Send, Code, Lightbulb, FileText, Zap} from 'lucide-vue-next';
import ModelSelector from './ModelSelector.vue';
import ReasoningSelector from './ReasoningSelector.vue';
import ToolSelector from './ToolSelector.vue';
import ContextLimitIndicator from './ContextLimitIndicator.vue';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const emit = defineEmits<{
	send: [content: string];
}>();

const chatStore = useChatStore();
const mainStore = useMainStore();

const inputMessage = ref('');
const inputRef = ref<HTMLTextAreaElement>();

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

const canSend = computed(() => inputMessage.value.trim().length > 0 && chatStore.selectedModel);

function handleSend(e?: Event) {
	if (e instanceof KeyboardEvent && e.shiftKey) return;
	e?.preventDefault();
	if (!canSend.value) return;
	emit('send', inputMessage.value.trim());
	inputMessage.value = '';
}

function selectHint(hint: {title: string; description: string; prompt?: string}) {
	if (!chatStore.selectedModel) return; // Don't send without a selected model
	const prompt = hint.prompt || `Help me ${hint.title.toLowerCase()}`;
	emit('send', prompt);
}

function autoResize() {
	const textarea = inputRef.value;
	if (!textarea) return;
	textarea.style.height = 'auto';
	textarea.style.height = Math.min(textarea.scrollHeight, 120) + 'px';
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
