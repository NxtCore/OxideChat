<template>
	<div class="flex flex-col gap-2">
		<div
			class="flex cursor-pointer items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none"
			@click="
				() => {
					userToggled = true;
					isExpanded = !isExpanded;
				}
			"
		>
			<Wrench class="h-3.5 w-3.5" />
			<span class="text-[10px] font-bold uppercase tracking-widest">{{ store.getTranslation('chat.tool_execution.tool') }}: {{ name }}</span>
			<span v-if="isExecuting" class="flex items-center gap-1 text-xs text-muted-foreground">
				<Loader2 class="h-3 w-3 animate-spin" />
				{{ store.getTranslation('chat.tool_execution.running') }}
			</span>
			<span v-else-if="error" class="flex items-center gap-1 text-xs text-destructive">
				<AlertCircle class="h-3 w-3" />
				{{ store.getTranslation('chat.tool_execution.failed') }}
			</span>
			<span v-else-if="output !== undefined" class="flex items-center gap-1 text-xs text-green-500">
				<CheckCircle class="h-3 w-3" />
				{{ store.getTranslation('chat.tool_execution.complete') }}
			</span>
			<ChevronDown class="ml-auto h-3 w-3 transition-transform" :class="isExpanded ? 'rotate-180' : ''" />
		</div>

		<Transition name="expand">
			<div v-if="isExpanded" class="w-full rounded-xl bg-muted/50 border px-4 py-3">
				<div class="mb-3">
					<span class="text-xs font-medium text-muted-foreground mb-1 block">{{ store.getTranslation('chat.tool_execution.arguments') }}</span>
					<div class="rounded-md bg-background/50 p-2 text-xs font-mono overflow-x-auto">
						<pre class="whitespace-pre-wrap">{{ formattedArgs }}</pre>
					</div>
				</div>

				<div v-if="imageUrl" class="mb-3">
					<span class="text-xs font-medium text-muted-foreground mb-1 block">{{
						store.getTranslation('chat.tool_execution.generated_image') || 'Generated Image'
					}}</span>
					<div class="relative group">
						<img
							:src="imageUrl"
							alt="Generated image"
							class="rounded-lg max-w-full max-h-96 object-contain cursor-pointer hover:opacity-90 transition-opacity"
							@click="openImageModal"
						/>
					</div>
				</div>

				<div v-else-if="output !== undefined" class="mb-3">
					<span class="text-xs font-medium text-muted-foreground mb-1 block">{{ store.getTranslation('chat.tool_execution.output') }}</span>
					<div class="rounded-md bg-background/50 p-2 text-xs font-mono overflow-x-auto max-h-64 overflow-y-auto">
						<pre class="whitespace-pre-wrap">{{ formattedOutput }}</pre>
					</div>
				</div>

				<div v-if="error" class="mb-3">
					<span class="text-xs font-medium text-destructive mb-1 block">{{ store.getTranslation('chat.tool_execution.error') }}</span>
					<div class="rounded-md bg-destructive/10 border border-destructive/20 p-2 text-xs text-destructive">
						{{ error }}
					</div>
				</div>

				<div v-if="durationMs" class="text-xs text-muted-foreground">{{ store.getTranslation('chat.tool_execution.completed_in', {ms: durationMs}) }}</div>
			</div>
		</Transition>

		<ImagePreview
			:is-open="showImageModal"
			:image-url="imageUrl"
			alt-text="Generated image"
			:filename="`generated-image-${props.id}.png`"
			@close="showImageModal = false"
		/>
	</div>
</template>

<script setup lang="ts">
import {Wrench, ChevronDown, Loader2, CheckCircle, AlertCircle} from 'lucide-vue-next';
import ImagePreview from '~/components/ImagePreview.vue';
import {useMainStore} from '~/stores';

const store = useMainStore();

const props = defineProps<{
	id: string;
	name: string;
	args?: Record<string, any> | string;
	output?: any;
	error?: string;
	isExecuting?: boolean;
	durationMs?: number;
}>();

const isExpanded = ref(props.isExecuting ?? false);
const showImageModal = ref(false);
const userToggled = ref(false);

watchEffect(() => {
	if (!props.isExecuting && props.output !== undefined && !userToggled.value) {
		isExpanded.value = false;
	}
});

const formattedArgs = computed(() => {
	if (typeof props.args === 'string') {
		try {
			return JSON.stringify(JSON.parse(props.args), null, 2);
		} catch {
			return props.args;
		}
	}
	return JSON.stringify(props.args || {}, null, 2);
});

const formattedOutput = computed(() => {
	if (typeof props.output === 'string') {
		return props.output;
	}
	return JSON.stringify(props.output, null, 2);
});

function isValidImageProtocol(url: string): boolean {
	if (url.startsWith('data:image/')) return true;
	if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('/')) return true;
	return false;
}

const imageUrl = computed(() => {
	if (!props.output) return null;
	const out = typeof props.output === 'string' ? tryParseJson(props.output) : props.output;
	if (out?.image_id && typeof out.image_id === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(out.image_id)) {
		return `/api/v1/images/${encodeURIComponent(out.image_id)}`;
	}
	if (out?.image_url && isValidImageProtocol(out.image_url)) return out.image_url;
	if (out?.image_reference && isValidImageProtocol(out.image_reference)) return out.image_reference;
	if (out?.url && isImageUrl(out.url)) return out.url;
	return null;
});

function tryParseJson(str: string): any {
	try {
		return JSON.parse(str);
	} catch {
		return null;
	}
}

function isImageUrl(url: string): boolean {
	if (url.startsWith('data:image/')) return true;
	const urlWithoutQuery = url.split('?')[0];
	if (!urlWithoutQuery) return false;
	const parts = urlWithoutQuery.split('.');
	const ext = parts[parts.length - 1]?.toLowerCase() || '';
	return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp'].includes(ext);
}

function openImageModal() {
	showImageModal.value = true;
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
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}

.fade-enter-active,
.fade-leave-active {
	transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
	opacity: 0;
}
</style>
