<template>
	<div class="p-0">
		<div class="mx-auto max-w-4xl">
			<div class="relative flex flex-col rounded-2xl border border-border bg-card shadow-sm">
				<div v-if="attachedImages.length > 0" class="flex gap-2 px-4 pt-3 flex-wrap bg-input/30">
					<div v-for="(img, idx) in attachedImages" :key="img.id" class="relative h-20 w-20 rounded-lg overflow-hidden border border-border group">
						<img :src="img.previewUrl" class="h-full w-full object-cover" alt="Attached image" />
						<button
							class="absolute top-1 right-1 h-5 w-5 rounded-full bg-destructive text-destructive-foreground opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center"
							@click="removeImage(idx)"
						>
							<X class="h-3 w-3" />
						</button>
					</div>
				</div>
				<ShadTextarea
					ref="textareaRef"
					v-model="message"
					:placeholder="placeholder"
					rows="1"
					class="min-h-15 w-full resize-none border-none px-4 py-3 text-foreground placeholder:text-muted-foreground focus-visible:ring-0"
					:disabled="chatStore.isStreaming"
					@keydown.enter.exact="handleEnter"
					@input="autoResize"
					@paste="handlePaste"
				/>

				<div class="flex items-center justify-between px-2 py-1">
					<div class="flex items-center gap-1">
						<button
							v-if="supportsImages"
							class="inline-flex h-8 w-8 items-center justify-center rounded-md hover:bg-muted transition-colors"
							:disabled="chatStore.isStreaming"
							@click="triggerImageUpload"
						>
							<ImageIcon class="h-4 w-4" />
						</button>
						<input ref="fileInputRef" type="file" accept="image/*" multiple class="hidden" @change="handleFileSelect" />
						<ModelSelector class="border-none! bg-transparent! shadow-none! hover:bg-muted/50" />
						<ReasoningSelector
							v-if="chatStore.hasReasoningCapability(chatStore.selectedModel)"
							class="border-none! bg-transparent! shadow-none! hover:bg-muted/50"
						/>
						<ToolSelector v-if="chatStore.hasToolCapability(chatStore.selectedModel)" />
						<ProviderSelector class="border-none! bg-transparent! shadow-none! hover:bg-muted/50" />
					</div>

					<div class="flex items-center gap-2">
						<ContextLimitIndicator />
						<ShadButton
							class="h-8 w-8 rounded-full bg-primary p-0 text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
							:disabled="!canSend"
							@click="sendMessage"
						>
							<ArrowUp v-if="!chatStore.isStreaming" class="h-4 w-4" />
							<Loader2 v-else class="h-4 w-4 animate-spin" />
						</ShadButton>
					</div>
				</div>
			</div>

			<p class="mt-2 text-center text-[10px] text-muted-foreground opacity-50">{{ store.getTranslation('chat.composer.hint') }}</p>
		</div>

		<McpManagerDialog v-model:open="chatStore.mcpManagerOpen" />
	</div>
</template>

<script setup lang="ts">
import {ArrowUp, Loader2, X, ImageIcon} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import ModelSelector from './ModelSelector.vue';
import ReasoningSelector from './ReasoningSelector.vue';
import ContextLimitIndicator from './ContextLimitIndicator.vue';
import ToolSelector from './ToolSelector.vue';
import ProviderSelector from './ProviderSelector.vue';
import McpManagerDialog from '../mcp/McpManagerDialog.vue';

const emit = defineEmits<{send: (content: string, parts?: any[]) => void}>();

const chatStore = useChatStore();
const store = useMainStore();
const message = ref('');
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const fileInputRef = ref<HTMLInputElement | null>(null);

interface AttachedImage {
	id: string;
	previewUrl: string;
	file?: File;
}

const attachedImages = ref<AttachedImage[]>([]);

onMounted(() => {
	if (chatStore.pendingBranchContent) {
		message.value = chatStore.pendingBranchContent;
		nextTick(() => autoResize());
	}
	if (chatStore.pendingBranchParts) {
		for (const part of chatStore.pendingBranchParts) {
			if (part.type === 'image' && part.image_id) {
				const config = useRuntimeConfig();
				const baseUrl = config.public.apiBase || '';
				attachedImages.value.push({
					id: part.image_id,
					previewUrl: `${baseUrl}/api/v1/images/${part.image_id}`,
				});
			}
		}
	}
	chatStore.clearPendingBranch();
});

const supportsImages = computed(() => {
	return chatStore.selectedModel?.input_modalities?.includes('IMAGE') ?? false;
});

const placeholder = computed(() => {
	if (chatStore.selectedModel) {
		return store.getTranslation('chat.composer.placeholder_model', {model: chatStore.selectedModel.display_name});
	}
	return store.getTranslation('chat.composer.placeholder_default');
});

const canSend = computed(() => {
	return (message.value.trim().length > 0 || attachedImages.value.length > 0) && chatStore.selectedModel && !chatStore.isStreaming;
});

function handleEnter(e: KeyboardEvent) {
	if (!e.shiftKey && canSend.value) {
		e.preventDefault();
		sendMessage();
	}
}

async function sendMessage() {
	if (!canSend.value) return;

	const parts: any[] = [];
	if (message.value.trim()) {
		parts.push({type: 'text', text: message.value.trim()});
	}

	for (const img of attachedImages.value) {
		parts.push({type: 'image', image_id: img.id});
	}

	emit('send', message.value.trim(), parts.length > 0 ? parts : undefined);
	message.value = '';
	for (const img of attachedImages.value) {
		URL.revokeObjectURL(img.previewUrl);
	}
	attachedImages.value = [];
	nextTick(() => autoResize());
}

function autoResize() {
	const textarea = textareaRef.value instanceof HTMLTextAreaElement ? textareaRef.value : textareaRef.value?.$el?.querySelector('textarea');
	if (!textarea) return;
	textarea.style.height = 'auto';
	textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}

function triggerImageUpload() {
	fileInputRef.value?.click();
}

async function handleFileSelect(event: Event) {
	const input = event.target as HTMLInputElement;
	if (!input.files) return;

	for (const file of Array.from(input.files)) {
		await uploadImage(file);
	}
	input.value = '';
}

async function handlePaste(event: ClipboardEvent) {
	if (!supportsImages.value) return;

	const items = event.clipboardData?.items;
	if (!items) return;

	for (const item of Array.from(items)) {
		if (item.type.startsWith('image/')) {
			event.preventDefault();
			const file = item.getAsFile();
			if (file) {
				await uploadImage(file);
			}
		}
	}
}

async function uploadImage(file: File) {
	return new Promise(async (resolve, reject) => {
		const reader = new FileReader();
		reader.onload = async () => {
			const dataUri = reader.result as string;
			const previewUrl = URL.createObjectURL(file);

			try {
				const config = useRuntimeConfig();
				const baseUrl = config.public.apiBase || '';
				const response = await fetch(`${baseUrl}/api/v1/images`, {
					method: 'POST',
					headers: {'Content-Type': 'application/json'},
					credentials: 'include',
					body: JSON.stringify({data_uri: dataUri}),
				});

				if (!response.ok) {
					URL.revokeObjectURL(previewUrl);
					reject(new Error('Upload failed'));
					return;
				}

				const result = await response.json();
				attachedImages.value.push({
					id: result.id,
					previewUrl,
					file,
				});
				resolve(undefined);
			} catch (error) {
				URL.revokeObjectURL(previewUrl);
				reject(error);
			}
		};
		reader.onerror = () => {
			reject(new Error('Failed to read file'));
		};
		reader.readAsDataURL(file);
	});
}

function removeImage(index: number) {
	const img = attachedImages.value[index];
	if (img) {
		URL.revokeObjectURL(img.previewUrl);
	}
	attachedImages.value.splice(index, 1);
}
</script>
