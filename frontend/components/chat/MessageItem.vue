<template>
	<div class="group flex gap-4" :class="isUser ? 'flex-row-reverse' : ''">
		<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full overflow-hidden bg-muted">
			<User v-if="isUser" class="h-4 w-4 text-muted-foreground" />
			<template v-else>
				<div v-if="providerIcon?.type === 'svg'" class="h-full w-full p-1.5 [&>svg]:h-full [&>svg]:w-full" v-html="providerIcon.icon" />
				<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" class="h-full w-full object-contain p-1.5" alt="Provider icon" />
				<Bot v-else class="h-4 w-4 text-muted-foreground" />
			</template>
		</div>

		<div class="flex flex-1 flex-col gap-2" :class="isUser ? 'items-end' : 'items-start'">
			<div
				v-if="!isUser && message.reasoning_content"
				class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none"
				@click="showReasoning = !showReasoning"
			>
				<Brain class="h-3.5 w-3.5 fill-current" />
				<span class="text-[10px] font-bold uppercase tracking-widest">{{ store.getTranslation('chat.message_item.reasoning') }}</span>
				<ChevronDown class="h-3 w-3 transition-transform" :class="showReasoning ? 'rotate-180' : ''" />
			</div>

			<Transition name="expand">
				<div v-if="showReasoning && (message.reasoning_content || isStreamingReasoning)" class="w-full max-w-3xl rounded-xl bg-muted/50 border p-4">
					<div class="prose prose-sm dark:prose-invert max-w-none opacity-80" v-html="renderedReasoning" @click="handleCodeBlockClick" />
					<div v-if="isStreamingReasoning && !message.reasoning_content" class="flex items-center gap-1 py-1">
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.15s" />
						<span class="h-1.5 w-1.5 animate-bounce rounded-full bg-primary/50" style="animation-delay: 0.3s" />
					</div>
				</div>
			</Transition>

			<div v-if="!isUser && message.tool_calls && message.tool_calls.length > 0" class="flex flex-col gap-2 mt-2 w-full max-w-3xl">
				<ToolExecutionDisplay
					v-for="tool in message.tool_calls"
					:key="tool.tool_call_id"
					:id="tool.tool_call_id"
					:name="tool.tool_name"
					:args="tool.input_args"
					:output="tool.output"
					:error="tool.error || undefined"
					:is-executing="!tool.output && !tool.error"
				/>
			</div>

			<div v-if="isUser && attachedImages.length > 0" class="flex gap-2 flex-wrap max-w-3xl mb-2">
				<div
					v-for="img in attachedImages"
					:key="img.image_id"
					class="rounded-lg overflow-hidden border border-border max-w-xs cursor-pointer hover:opacity-90 transition-opacity"
					@click="openImagePreview(img.url, `attached-${img.image_id}.png`)"
				>
					<img :src="img.url" class="max-h-64 object-contain" :alt="`Attached image ${img.image_id}`" />
				</div>
			</div>

			<!-- User message content with edit button -->
			<div v-if="isUser && (message.content || !isStreaming)" class="flex flex-col gap-2">
				<div
					v-if="!isEditing"
					class="prose prose-sm md:prose-base dark:prose-invert max-w-3xl rounded-xl bg-muted/50 px-4 py-2 text-foreground"
					v-html="renderedContent"
					@click="handleCodeBlockClick"
				/>
				<div v-else class="w-full max-w-3xl">
					<textarea
						ref="editTextarea"
						v-model="editContent"
						class="w-full min-h-[100px] rounded-xl bg-muted/50 px-4 py-2 text-foreground border border-border focus:border-primary focus:outline-none resize-none"
						@keydown.escape="cancelEdit"
					/>
					<div class="flex gap-2 mt-2 justify-end">
						<ShadButton variant="ghost" size="sm" @click="cancelEdit">Cancel</ShadButton>
						<ShadButton size="sm" @click="saveEdit">Save & Fork</ShadButton>
					</div>
				</div>
				<!-- User message action bar (matches assistant style) -->
				<div
					v-if="!isEditing && !isStreaming"
					class="flex items-center gap-0.5 rounded-lg border border-border/50 bg-popover/80 backdrop-blur-sm p-0.5 shadow-lg opacity-0 transition-opacity group-hover:opacity-100 self-end"
				>
					<!-- Fork Navigator for user messages -->
					<template v-if="message.sibling_count > 1">
						<ShadButton
							variant="ghost"
							size="icon"
							class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50"
							:disabled="message.fork_index <= 1"
							@click="handleForkNavigation('prev')"
						>
							<ChevronLeft class="h-3.5 w-3.5" />
						</ShadButton>
						<span class="min-w-[2.5rem] text-center text-xs tabular-nums text-muted-foreground select-none"
							>{{ message.fork_index }}/{{ message.sibling_count }}</span
						>
						<ShadButton
							variant="ghost"
							size="icon"
							class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50"
							:disabled="message.fork_index >= message.sibling_count"
							@click="handleForkNavigation('next')"
						>
							<ChevronRight class="h-3.5 w-3.5" />
						</ShadButton>
						<div class="w-px h-4 bg-border/50 mx-0.5" />
					</template>
					<ShadTooltip>
						<ShadTooltipTrigger as-child>
							<ShadButton variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50" @click="startEdit">
								<Pencil class="h-3.5 w-3.5" />
							</ShadButton>
						</ShadTooltipTrigger>
						<ShadTooltipContent side="top" :side-offset="8">
							<p class="text-xs">Edit message</p>
						</ShadTooltipContent>
					</ShadTooltip>
					<ShadTooltip>
						<ShadTooltipTrigger as-child>
							<ShadButton
								variant="ghost"
								size="icon"
								class="h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-accent/50"
								@click="copyUserContent"
							>
								<Check v-if="userCopied" class="h-3.5 w-3.5 text-primary" />
								<Copy v-else class="h-3.5 w-3.5" />
							</ShadButton>
						</ShadTooltipTrigger>
						<ShadTooltipContent side="top" :side-offset="8">
							<p class="text-xs">{{ userCopied ? 'Copied' : 'Copy message' }}</p>
						</ShadTooltipContent>
					</ShadTooltip>
				</div>
			</div>

			<!-- Assistant message content -->
			<div
				v-else-if="!isUser && (message.content || !isStreaming)"
				class="prose prose-sm md:prose-base dark:prose-invert max-w-3xl"
				v-html="renderedContent"
				@click="handleCodeBlockClick"
			/>

			<div v-else-if="!isUser && isStreaming && !message.content" class="flex items-center gap-1 py-2">
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.15s" />
				<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/50" style="animation-delay: 0.3s" />
			</div>
			<div v-if="!isUser && !isStreaming" class="flex items-center gap-3 mt-1">
				<span v-if="modelDisplayName" class="text-xs text-muted-foreground">
					{{ modelDisplayName }}
				</span>
				<MessageActions
					:message="message"
					:model-name="modelDisplayName || undefined"
					:can-regenerate="isLastAssistantMessage"
					:current-index="message.fork_index"
					:sibling-count="message.sibling_count"
					class="opacity-0 transition-opacity group-hover:opacity-100"
					@navigate="handleForkNavigation"
				/>
			</div>

			<CodePreview v-if="previewData" :code="previewData.code" :language="previewData.language" :is-open="!!previewData" @close="closePreview" />
			<ImagePreview :is-open="showImagePreview" :image-url="imagePreviewUrl" :filename="imagePreviewFilename" @close="closeImagePreview" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {User, Bot, Brain, ChevronDown, ChevronLeft, ChevronRight, Pencil, Copy, Check} from 'lucide-vue-next';
import MessageActions from './MessageActions.vue';
import CodePreview from './CodePreview.vue';
import ImagePreview from '~/components/ImagePreview.vue';
import ToolExecutionDisplay from './ToolExecutionDisplay.vue';
import type {ChatMessage} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useIconsStore} from '~/stores/icons';
import {useMainStore} from '~/stores';
import {useMarkdown, extractCodeForPreview, ICON_COPY, ICON_CHECK} from '~/composables/useMarkdown';

const store = useMainStore();

const props = defineProps<{
	message: ChatMessage;
	animation?: string;
}>();

const chatStore = useChatStore();
const iconStore = useIconsStore();
const {renderStreaming, renderComplete} = useMarkdown();

const showReasoning = ref(false);
const previewData = ref<{code: string; language: string} | null>(null);
const showImagePreview = ref(false);
const imagePreviewUrl = ref<string | null>(null);
const imagePreviewFilename = ref<string | undefined>(undefined);

// Edit state
const isEditing = ref(false);
const editContent = ref('');
const editTextarea = ref<HTMLTextAreaElement | null>(null);
const userCopied = ref(false);

const isUser = computed(() => props.message.role === 'user');
const isStreaming = computed(() => props.message.id.startsWith('streaming-'));
const isStreamingReasoning = computed(() => isStreaming.value && chatStore.isStreaming && !props.message.content);

const attachedImages = computed(() => {
	if (!props.message.content_parts || !Array.isArray(props.message.content_parts)) {
		return [];
	}
	return props.message.content_parts
		.filter((part: any) => part.type === 'image' && part.image_id)
		.map((part: any) => ({
			image_id: part.image_id,
			url: `/api/v1/images/${part.image_id}`,
		}));
});

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
	[() => props.message.reasoning_content, () => props.message.content],
	([, newContent]) => {
		if (props.message.reasoning_content && isStreaming.value && !props.message.content) {
			showReasoning.value = true;
		}
		if (newContent && showReasoning.value) {
			showReasoning.value = false;
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

	const copyBtn = target.closest('.code-block-copy-btn') as HTMLElement | null;
	const previewBtn = target.closest('.code-block-preview-btn') as HTMLElement | null;

	if (copyBtn) {
		const wrapper = copyBtn.closest('.code-block-wrapper');
		const codeEl = wrapper?.querySelector('code');
		if (codeEl) {
			navigator.clipboard.writeText(codeEl.textContent || '');
			copyBtn.classList.add('copied');
			copyBtn.innerHTML = ICON_CHECK;
			setTimeout(() => {
				copyBtn.classList.remove('copied');
				copyBtn.innerHTML = ICON_COPY;
			}, 2000);
		}
	}

	if (previewBtn) {
		const result = extractCodeForPreview(previewBtn);
		if (result) {
			previewData.value = result;
		}
	}

	const imgTag = target.closest('img') as HTMLImageElement | null;
	if (imgTag && imgTag.src) {
		const alt = imgTag.alt || 'image';
		const filename = `${alt.replace(/\s+/g, '-').replace(/[^a-zA-Z0-9-_]/g, '')}.png`;
		openImagePreview(imgTag.src, filename);
	}
}

function closePreview() {
	previewData.value = null;
}

function openImagePreview(url: string, filename?: string) {
	imagePreviewUrl.value = url;
	imagePreviewFilename.value = filename;
	showImagePreview.value = true;
}

function closeImagePreview() {
	showImagePreview.value = false;
	imagePreviewUrl.value = null;
	imagePreviewFilename.value = undefined;
}

function handleForkNavigation(direction: 'prev' | 'next') {
	const newIndex = direction === 'prev' ? props.message.fork_index - 1 : props.message.fork_index + 1;
	if (chatStore.activeChat) {
		chatStore.switchFork(chatStore.activeChat.id, props.message.id, newIndex);
	}
}

async function copyUserContent() {
	if (!props.message.content) return;
	await navigator.clipboard.writeText(props.message.content);
	userCopied.value = true;
	setTimeout(() => {
		userCopied.value = false;
	}, 2000);
}

function startEdit() {
	editContent.value = props.message.content;
	isEditing.value = true;
	nextTick(() => {
		editTextarea.value?.focus();
	});
}

function cancelEdit() {
	isEditing.value = false;
	editContent.value = '';
}

async function saveEdit() {
	if (!chatStore.activeChat || editContent.value === props.message.content) {
		cancelEdit();
		return;
	}

	const chatId = chatStore.activeChat.id;
	const newContent = editContent.value;

	// Create the fork with edited content
	const newMessage = await chatStore.editMessage(chatId, props.message.id, newContent);
	cancelEdit();

	if (!newMessage) {
		return;
	}

	// Reload the chat to get the new fork path (messages up to the edited message)
	await chatStore.fetchChat(chatId);

	// Trigger a new generation with the edited message
	// Pass skipUserMessage=true since the edited message already exists in the database
	await chatStore.sendAndStream(chatId, newContent, undefined, true);
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

.prose :deep(img) {
	cursor: pointer;
	transition: opacity 0.2s;
}

.prose :deep(img:hover) {
	opacity: 0.9;
}
</style>
