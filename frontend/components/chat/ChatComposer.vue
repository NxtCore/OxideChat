<template>
	<div class="p-0">
		<div class="mx-auto max-w-4xl">
			<div
				v-if="modelStore.presetFallbackNotice"
				data-testid="preset-fallback-notice"
				class="mb-2 flex items-center gap-2 rounded-lg border border-border bg-muted/50 px-3 py-2 text-xs text-muted-foreground"
			>
				<Info class="h-3.5 w-3.5 shrink-0" />
				<span class="flex-1">{{ store.getTranslation(modelStore.presetFallbackNotice.key, {model: modelStore.presetFallbackNotice.model}) }}</span>
				<button class="rounded p-0.5 hover:bg-accent" :aria-label="store.getTranslation('presets.dismiss')" @click="modelStore.presetFallbackNotice = null">
					<X class="h-3.5 w-3.5" />
				</button>
			</div>
			<div
				v-if="arenaPending"
				data-testid="arena-pick-to-continue"
				class="mb-2 flex items-center gap-2 rounded-lg border border-border bg-muted/50 px-3 py-2 text-xs text-muted-foreground"
			>
				<Swords class="h-3.5 w-3.5 shrink-0" />
				<span class="flex-1">{{ store.getTranslation('chat.arena.pick_to_continue') }}</span>
			</div>
			<div
				data-testid="chat-composer"
				class="relative flex flex-col rounded-2xl border bg-card shadow-sm transition-colors"
				:class="isDragging ? 'border-primary ring-2 ring-primary/30' : 'border-border'"
				@dragenter.prevent="handleDragEnter"
				@dragover.prevent="handleDragOver"
				@dragleave="handleDragLeave"
				@drop.prevent="handleDrop"
			>
				<div v-if="attachedImages.length > 0" class="flex gap-2 px-4 pt-3 flex-wrap bg-input/30">
					<div v-for="(img, idx) in attachedImages" :key="img.id" data-testid="attached-image" class="relative h-20 w-20 rounded-lg overflow-hidden border border-border group">
						<img :src="img.previewUrl" class="h-full w-full object-cover" alt="Attached image" />
						<button
							class="absolute top-1 right-1 h-5 w-5 rounded-full bg-destructive text-destructive-foreground opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center"
							@click="removeImage(idx)"
						>
							<X class="h-3 w-3" />
						</button>
					</div>
				</div>
				<div v-if="attachedFiles.length > 0 || uploadingFiles.length > 0" class="flex gap-2 px-4 pt-3 flex-wrap">
					<div
						v-for="(file, idx) in attachedFiles"
						:key="file.id"
						data-testid="attached-file"
						class="flex max-w-60 items-center gap-1.5 rounded-lg border border-border bg-muted/50 py-1 pl-2 pr-1 text-xs"
						:title="file.token_count ? store.getTranslation('chat.composer.attachment_tokens', {count: file.token_count.toLocaleString()}) : file.filename"
					>
						<FileText class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
						<span class="truncate">{{ file.filename }}</span>
						<button
							type="button"
							class="rounded p-0.5 hover:bg-accent"
							:aria-label="store.getTranslation('chat.composer.remove_attachment', {name: file.filename})"
							@click="attachedFiles.splice(idx, 1)"
						>
							<X class="h-3 w-3" />
						</button>
					</div>
					<div
						v-for="file in uploadingFiles"
						:key="file.key"
						data-testid="attached-file-uploading"
						class="flex max-w-60 items-center gap-1.5 rounded-lg border border-border bg-muted/50 px-2 py-1 text-xs text-muted-foreground"
					>
						<Loader2 class="h-3.5 w-3.5 shrink-0 animate-spin" />
						<span class="truncate">{{ store.getTranslation('chat.attachments.uploading', {name: file.filename}) }}</span>
					</div>
				</div>
				<ShadTextarea
					ref="textareaRef"
					v-model="message"
					:placeholder="placeholder"
					rows="1"
					class="min-h-15 w-full resize-none border-none px-4 py-3 text-foreground placeholder:text-muted-foreground focus-visible:ring-0"
					:disabled="chatStore.isStreaming || arenaPending"
					@keydown.enter.exact="handleEnter"
					@input="autoResize"
					@paste="handlePaste"
				/>

				<div class="flex items-center justify-between gap-1 px-2 py-1">
					<div class="flex min-w-0 flex-1 items-center gap-1">
						<button
							v-if="supportsImages"
							type="button"
							data-testid="composer-attach-image"
							class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md hover:bg-muted transition-colors"
							:aria-label="store.getTranslation('chat.composer.attach_image')"
							:title="store.getTranslation('chat.composer.attach_image')"
							:disabled="chatStore.isStreaming"
							@click="triggerImageUpload"
						>
							<ImageIcon class="h-4 w-4" />
						</button>
						<input ref="fileInputRef" type="file" accept="image/*" multiple class="hidden" @change="handleFileSelect" />
						<button
							type="button"
							data-testid="composer-attach-file"
							class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md hover:bg-muted transition-colors"
							:aria-label="store.getTranslation('chat.composer.attach_file')"
							:title="store.getTranslation('chat.composer.attach_file')"
							:disabled="chatStore.isStreaming"
							@click="documentInputRef?.click()"
						>
							<Paperclip class="h-4 w-4" />
						</button>
						<input ref="documentInputRef" data-testid="composer-file-input" type="file" :accept="ATTACHMENT_ACCEPT" multiple class="hidden" @change="handleDocumentSelect" />
						<ModelSelector class="border-none! bg-transparent! shadow-none! hover:bg-muted/50" />
						<template v-if="!arenaSelected">
							<ToolSelector v-if="modelStore.hasToolCapability(modelStore.selectedModel)" class="shrink-0" />
							<ChatSettingsMenu />
						</template>
					</div>

					<div class="flex shrink-0 items-center gap-1.5">
						<PresetLevelSelector />
						<ContextLimitIndicator />
						<ShadButton
							data-testid="composer-send"
							class="h-8 w-8 rounded-full bg-primary p-0 text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-50"
							:aria-label="store.getTranslation('chat.composer.send')"
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

		<McpManagerDialog v-model:open="preferencesStore.mcpManagerOpen" />
	</div>
</template>

<script setup lang="ts">
import {ArrowUp, Loader2, X, ImageIcon, Info, Swords, Paperclip, FileText} from 'lucide-vue-next';
import {ATTACHMENT_ACCEPT, FILE_MAX_BYTES, fileErrorMessage, postFile} from '~/composables/api/files';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {usePreferencesStore} from '~/stores/preferencesStore';
import {useMainStore} from '~/stores';
import ModelSelector from './ModelSelector.vue';
import PresetLevelSelector from './PresetLevelSelector.vue';
import ContextLimitIndicator from './ContextLimitIndicator.vue';
import ToolSelector from './ToolSelector.vue';
import ChatSettingsMenu from './ChatSettingsMenu.vue';
import McpManagerDialog from '../mcp/McpManagerDialog.vue';

const emit = defineEmits<{send: (content: string, parts?: any[]) => void}>();

const chatStore = useChatStore();
const modelStore = useModelStore();
const preferencesStore = usePreferencesStore();
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
const documentInputRef = ref<HTMLInputElement | null>(null);
const attachedFiles = ref<{id: string; filename: string; token_count: number}[]>([]);
const uploadingFiles = ref<{key: string; filename: string}[]>([]);
const isDragging = ref(false);
let dragDepth = 0;

function restorePending() {
	if (!chatStore.pendingBranchContent && !chatStore.pendingBranchParts) return;
	if (chatStore.pendingBranchContent && !message.value.trim()) {
		message.value = chatStore.pendingBranchContent;
		nextTick(() => autoResize());
	}
	for (const part of chatStore.pendingBranchParts ?? []) {
		if (part.type === 'image' && part.image_id && !attachedImages.value.some(img => img.id === part.image_id)) {
			attachedImages.value.push({
				id: part.image_id,
				previewUrl: fileUrl(part.image_id, useRuntimeConfig().public.apiBase || ''),
			});
		}
		if (part.type === 'file' && part.attachment_id && !attachedFiles.value.some(file => file.id === part.attachment_id)) {
			attachedFiles.value.push({id: part.attachment_id, filename: part.filename, token_count: 0});
		}
	}
	chatStore.clearPendingBranch();
}

onMounted(restorePending);
watch(() => [chatStore.pendingBranchContent, chatStore.pendingBranchParts], restorePending);

const supportsImages = computed(() => {
	return modelStore.selectedModel?.input_modalities?.includes('IMAGE') ?? false;
});

const placeholder = computed(() => {
	if (modelStore.selectedModel) {
		return store.getTranslation('chat.composer.placeholder_model', {model: modelStore.selectedModel.display_name});
	}
	return store.getTranslation('chat.composer.placeholder_default');
});

const arenaSelected = computed(() => isArena(modelStore.selectedModel?.preset));
const arenaPending = computed(() => !!chatStore.arenaRound && !chatStore.arenaRound.decided && !chatStore.isStreaming);

const canSend = computed(() => {
	const hasContent = message.value.trim().length > 0 || attachedImages.value.length > 0 || attachedFiles.value.length > 0;
	return hasContent && uploadingFiles.value.length === 0 && modelStore.selectedModel && !chatStore.isStreaming && !arenaPending.value;
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
	for (const file of attachedFiles.value) {
		parts.push({type: 'file', attachment_id: file.id, filename: file.filename});
	}

	emit('send', message.value.trim(), parts.length > 0 ? parts : undefined);
	message.value = '';
	for (const img of attachedImages.value) {
		URL.revokeObjectURL(img.previewUrl);
	}
	attachedImages.value = [];
	attachedFiles.value = [];
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

async function handleDocumentSelect(event: Event) {
	const input = event.target as HTMLInputElement;
	const files = Array.from(input.files ?? []);
	input.value = '';
	await Promise.all(files.map(uploadFile));
}

async function uploadFile(file: File) {
	if (file.size > FILE_MAX_BYTES) {
		store.toast(store.getTranslation('files.error.too_large'), {type: 'error'});
		return;
	}
	const pending = {key: crypto.randomUUID(), filename: file.name};
	uploadingFiles.value.push(pending);
	try {
		const stored = await postFile(file, 'chat_attachment');
		attachedFiles.value.push({id: stored.id, filename: stored.filename ?? file.name, token_count: stored.token_count ?? 0});
	} catch (error) {
		store.toast(fileErrorMessage(error), {type: 'error'});
	} finally {
		uploadingFiles.value = uploadingFiles.value.filter(item => item.key !== pending.key);
	}
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

function hasFiles(event: DragEvent) {
	return event.dataTransfer?.types.includes('Files') ?? false;
}

function handleDragEnter(event: DragEvent) {
	if (!hasFiles(event) || chatStore.isStreaming) return;
	dragDepth++;
	isDragging.value = true;
}

function handleDragOver(event: DragEvent) {
	if (!event.dataTransfer) return;
	event.dataTransfer.dropEffect = hasFiles(event) && !chatStore.isStreaming ? 'copy' : 'none';
}

function handleDragLeave() {
	if (!isDragging.value) return;
	dragDepth = Math.max(0, dragDepth - 1);
	if (dragDepth === 0) isDragging.value = false;
}

async function handleDrop(event: DragEvent) {
	dragDepth = 0;
	isDragging.value = false;
	if (chatStore.isStreaming) return;

	const files = Array.from(event.dataTransfer?.files ?? []);
	const images = files.filter(file => file.type.startsWith('image/'));
	if (images.length > 0 && !supportsImages.value) store.toast(store.getTranslation('chat.composer.images_not_supported'), {type: 'error'});
	await Promise.all(files.filter(file => !images.includes(file)).map(uploadFile));
	if (!supportsImages.value) return;
	for (const file of images) {
		await uploadImage(file);
	}
}

async function uploadImage(file: File) {
	if (file.size > FILE_MAX_BYTES) {
		store.toast(store.getTranslation('files.error.too_large'), {type: 'error'});
		return;
	}
	const previewUrl = URL.createObjectURL(file);
	try {
		const stored = await postFile(file, 'image');
		attachedImages.value.push({id: stored.id, previewUrl, file});
	} catch (error) {
		URL.revokeObjectURL(previewUrl);
		store.toast(fileErrorMessage(error), {type: 'error'});
	}
}

function removeImage(index: number) {
	const img = attachedImages.value[index];
	if (img) {
		URL.revokeObjectURL(img.previewUrl);
	}
	attachedImages.value.splice(index, 1);
}
</script>
