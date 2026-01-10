<template>
	<div class="flex gap-3">
		<!-- Avatar -->
		<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
			<Bot class="h-4 w-4" />
		</div>

		<!-- Streaming content -->
		<div class="flex max-w-[80%] flex-col gap-1">
			<span class="text-xs text-muted-foreground">{{ store.getTranslation('chat.streaming_message.assistant') }}</span>

			<div
				v-if="reasoningContent"
				class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none"
				@click="showReasoning = !showReasoning"
			>
				<Brain class="h-3.5 w-3.5 fill-current" />
				<span class="text-[10px] font-bold uppercase tracking-widest">{{ store.getTranslation('chat.message_item.reasoning') }}</span>
				<ChevronDown class="h-3 w-3 transition-transform" :class="showReasoning ? 'rotate-180' : ''" />
			</div>

			<Transition name="expand">
				<div v-if="showReasoning && reasoningContent" class="w-full max-w-3xl rounded-xl bg-muted/50 border p-4">
					<div class="prose prose-sm dark:prose-invert max-w-none opacity-80" v-html="renderedReasoning" @click="handleCodeBlockClick" />
					<div v-if="!reasoningContent" class="flex items-center gap-1 py-1">
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.15s" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.3s" />
					</div>
				</div>
			</Transition>

			<div v-if="Object.keys(toolCalls).length > 0" class="flex flex-col gap-2 mt-2">
				<ToolExecutionDisplay
					v-for="(tool, id) in toolCalls"
					:key="id"
					:id="String(id)"
					:name="tool.name"
					:args="tool.args"
					:output="tool.output"
					:error="tool.error"
					:is-executing="tool.isExecuting"
				/>
			</div>

			<div class="relative rounded-2xl rounded-bl-md bg-muted px-4 py-3 text-foreground">
				<div v-if="!content && Object.keys(toolCalls).length === 0" class="flex items-center gap-1">
					<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" />
					<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.15s" />
					<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.3s" />
				</div>

				<div
					v-else-if="content"
					class="prose prose-sm dark:prose-invert max-w-none"
					:class="animationClass"
					v-html="renderedContent"
					@click="handleCodeBlockClick"
				/>
			</div>
		</div>
	</div>

	<CodePreview v-if="previewData" :code="previewData.code" :language="previewData.language" :is-open="!!previewData" @close="closePreview" />
</template>

<script setup lang="ts">
import {Bot, Brain, ChevronDown} from 'lucide-vue-next';
import CodePreview from './CodePreview.vue';
import ToolExecutionDisplay from './ToolExecutionDisplay.vue';
import {useMarkdown, extractCodeForPreview, ICON_COPY, ICON_CHECK} from '~/composables/useMarkdown';
import {useMainStore} from '~/stores';

interface ToolCallState {
	name: string;
	args: string;
	output?: any;
	error?: string;
	isExecuting: boolean;
}

const store = useMainStore();

const props = defineProps<{
	animation?: 'fade' | 'typewriter' | 'slide' | 'none';
}>();

const content = ref('');
const reasoningContent = ref('');
const showReasoning = ref(false);
const previewData = ref<{code: string; language: string} | null>(null);
const toolCalls = ref<Record<string, ToolCallState>>({});

const {renderStreaming} = useMarkdown();

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
	return renderStreaming(content.value);
});

const renderedReasoning = computed(() => {
	return renderStreaming(reasoningContent.value);
});

watch(
	() => reasoningContent.value,
	newVal => {
		if (newVal && !content.value) {
			showReasoning.value = true;
		}
	},
	{immediate: true}
);

function handleCodeBlockClick(event: MouseEvent) {
	const target = event.target as HTMLElement;

	if (target.classList.contains('code-block-copy-btn')) {
		const wrapper = target.closest('.code-block-wrapper');
		const codeEl = wrapper?.querySelector('code');
		if (codeEl) {
			navigator.clipboard.writeText(codeEl.textContent || '');
			target.classList.add('copied');
			target.innerHTML = ICON_CHECK;
			setTimeout(() => {
				target.classList.remove('copied');
				target.innerHTML = ICON_COPY;
			}, 2000);
		}
	}

	if (target.classList.contains('code-block-preview-btn')) {
		const result = extractCodeForPreview(target);
		if (result) {
			previewData.value = result;
		}
	}
}

function closePreview() {
	previewData.value = null;
}

defineExpose({
	setContent: (text: string) => {
		content.value = text;
	},
	appendContent: (text: string) => {
		content.value += text;
	},
	setReasoningContent: (text: string) => {
		reasoningContent.value = text;
	},
	appendReasoningContent: (text: string) => {
		reasoningContent.value += text;
	},
	getContent: () => content.value,
	getReasoningContent: () => reasoningContent.value,
	startToolCall: (id: string, name: string) => {
		toolCalls.value[id] = {
			name,
			args: '',
			isExecuting: false,
		};
	},
	appendToolCallArgs: (id: string, argsDelta: string) => {
		if (toolCalls.value[id]) {
			toolCalls.value[id].args += argsDelta;
		}
	},
	endToolCall: (id: string) => {
		if (toolCalls.value[id]) {
			toolCalls.value[id].isExecuting = true;
		}
	},
	setToolResult: (id: string, output: any, error?: string) => {
		if (toolCalls.value[id]) {
			toolCalls.value[id].output = output;
			toolCalls.value[id].error = error;
			toolCalls.value[id].isExecuting = false;
		}
	},
	getToolCalls: () => toolCalls.value,
});
</script>

<style scoped>
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

.expand-enter-active,
.expand-leave-active {
	transition: all 0.3s ease;
	overflow: hidden;
}

.expand-enter-from,
.expand-leave-to {
	opacity: 0;
	max-height: 0;
	padding-top: 0;
	padding-bottom: 0;
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}
</style>
