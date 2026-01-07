<template>
	<div class="group flex gap-4 py-6" :class="isUser ? 'flex-row-reverse' : ''">
		<!-- Avatar -->
		<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full overflow-hidden bg-muted">
			<User v-if="isUser" class="h-4 w-4 text-muted-foreground" />
			<template v-else>
				<div v-if="providerIcon?.type === 'svg'" class="h-full w-full p-1.5 [&>svg]:h-full [&>svg]:w-full" v-html="providerIcon.icon" />
				<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" class="h-full w-full object-contain p-1.5" alt="Provider icon" />
				<Bot v-else class="h-4 w-4 text-muted-foreground" />
			</template>
		</div>

		<!-- Message content -->
		<div class="flex flex-1 flex-col gap-2" :class="isUser ? 'items-end' : 'items-start'">
			<!-- Reasoning toggle (for assistant messages with reasoning) -->
			<div
				v-if="!isUser && (message.reasoning_content || isStreamingReasoning)"
				class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none"
				@click="showReasoning = !showReasoning"
			>
				<Brain class="h-3.5 w-3.5 fill-current" />
				<span class="text-[10px] font-bold uppercase tracking-widest">Reasoning</span>
				<ChevronDown class="h-3 w-3 transition-transform" :class="showReasoning ? 'rotate-180' : ''" />
			</div>

			<!-- Reasoning content -->
			<Transition name="expand">
				<div v-if="showReasoning && (message.reasoning_content || isStreamingReasoning)" class="w-full max-w-3xl rounded-xl bg-muted/50 border p-4">
					<div class="prose prose-sm dark:prose-invert max-w-none opacity-80" v-html="renderedReasoning" @click="handleCodeBlockClick" />
					<!-- Typing indicator for reasoning -->
					<div v-if="isStreamingReasoning && !message.reasoning_content" class="flex items-center gap-1 py-1">
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.15s" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.3s" />
					</div>
				</div>
			</Transition>

			<!-- Main content -->
			<div
				v-if="message.content || !isStreaming"
				class="prose prose-sm md:prose-base dark:prose-invert max-w-3xl"
				:class="isUser ? 'rounded-xl bg-muted/50 px-4 py-2 text-foreground' : ''"
				v-html="renderedContent"
				@click="handleCodeBlockClick"
			/>

			<!-- Typing indicator (for streaming assistant message with no content yet) -->
			<div v-else-if="!isUser && isStreaming && !message.content" class="flex items-center gap-1 py-2">
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.15s" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.3s" />
			</div>

			<!-- Model name and actions row -->
			<div v-if="!isUser && !isStreaming" class="flex items-center gap-3 mt-1">
				<!-- Model name badge -->
				<span v-if="modelDisplayName" class="text-xs text-muted-foreground">
					{{ modelDisplayName }}
				</span>

				<!-- Message actions -->
				<MessageActions
					:message="message"
					:model-name="modelDisplayName"
					:can-regenerate="isLastAssistantMessage"
					class="opacity-0 transition-opacity group-hover:opacity-100"
				/>
			</div>

			<!-- Preview Modal -->
			<CodePreview v-if="previewData" :code="previewData.code" :language="previewData.language" :is-open="!!previewData" @close="closePreview" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {User, Bot, Brain, ChevronDown} from 'lucide-vue-next';
import MessageActions from './MessageActions.vue';
import CodePreview from './CodePreview.vue';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useIconsStore} from '~/stores/icons';
import {useMarkdown, extractCodeForPreview, ICON_COPY, ICON_CHECK} from '~/composables/useMarkdown';

const props = defineProps<{
	message: ChatMessage;
	animation?: string;
}>();

const chatStore = useChatStore();
const iconStore = useIconsStore();
const {renderStreaming, renderComplete} = useMarkdown();

const showReasoning = ref(false);
const previewData = ref<{code: string; language: string} | null>(null);

const isUser = computed(() => props.message.role === 'user');
const isStreaming = computed(() => props.message.id.startsWith('streaming-'));
const isStreamingReasoning = computed(() => isStreaming.value && chatStore.isStreaming && !props.message.content);

const model = computed(() => {
	if (isUser.value) return null;
	return chatStore.models.find(m => m.id === props.message.model_id);
});

const modelDisplayName = computed(() => {
	if (!model.value) return null;
	if (model.value.display_name) return model.value.display_name;
	const modelId = props.message.model_id;
	if (modelId?.includes(':')) {
		return modelId.split(':')[1];
	}
	return modelId;
});

const isLastAssistantMessage = computed(() => {
	const messages = chatStore.messages;
	const lastAssistant = [...messages].reverse().find(m => m.role === 'assistant');
	return lastAssistant?.id === props.message.id;
});

const providerIcon = computed(() => {
	if (!model.value) return null;
	if (model.value.provider_icon_svg) {
		return {type: 'svg' as const, icon: model.value.provider_icon_svg};
	}
	return iconStore.getProviderIcon(model.value.provider_name);
});

watch(
	() => props.message.reasoning_content,
	newVal => {
		if (newVal && isStreaming.value && !props.message.content) {
			showReasoning.value = true;
		}
	},
	{immediate: true}
);

const renderedContent = computed(() => {
	if (!props.message.content) return '';
	if (isStreaming.value) {
		return renderStreaming(props.message.content);
	}
	return renderComplete(props.message.content);
});

const renderedReasoning = computed(() => {
	if (!props.message.reasoning_content) return '';

	if (isStreaming.value) {
		return renderStreaming(props.message.reasoning_content);
	}
	return renderComplete(props.message.reasoning_content);
});

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
</script>

<style scoped>
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
	margin-bottom: 0;
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}
</style>
