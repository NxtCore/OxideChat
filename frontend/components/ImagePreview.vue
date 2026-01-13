<template>
	<Teleport to="body">
		<Transition name="fade">
			<div v-if="isOpen && imageUrl" class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm" @click="handleClose">
				<div class="relative max-w-[90vw] max-h-[90vh]" @click.stop>
					<img :src="imageUrl" :alt="altText" class="max-w-full max-h-[90vh] object-contain rounded-lg" />
					<button class="absolute top-4 right-4 p-2 rounded-lg bg-background/80 backdrop-blur-sm hover:bg-background transition-colors" @click="handleClose">
						<X class="h-5 w-5" />
					</button>
					<div class="absolute bottom-4 left-1/2 -translate-x-1/2 flex gap-2">
						<button
							class="px-4 py-2 rounded-lg bg-background/80 backdrop-blur-sm hover:bg-background transition-colors flex items-center gap-2"
							@click="handleDownload"
						>
							<Download class="h-4 w-4" />
							<span class="text-sm">{{ downloadLabel }}</span>
						</button>
						<button
							class="px-4 py-2 rounded-lg bg-background/80 backdrop-blur-sm hover:bg-background transition-colors flex items-center gap-2"
							@click="handleCopy"
						>
							<Copy v-if="!copied" class="h-4 w-4" />
							<Check v-else class="h-4 w-4" />
							<span class="text-sm">{{ copyLabel }}</span>
						</button>
					</div>
				</div>
			</div>
		</Transition>
	</Teleport>
</template>

<script setup lang="ts">
import {Download, Copy, X, Check} from 'lucide-vue-next';
import {useMainStore} from '~/stores';

const store = useMainStore();

const props = defineProps<{
	isOpen: boolean;
	imageUrl: string | null;
	altText?: string;
	filename?: string;
}>();

const emit = defineEmits<{
	close: [];
}>();

const copied = ref(false);

const downloadLabel = computed(() => store.getTranslation('chat.image_preview.download') || 'Download');
const copyLabel = computed(() => {
	if (copied.value) return store.getTranslation('chat.image_preview.copied') || 'Copied!';
	return store.getTranslation('chat.image_preview.copy') || 'Copy URL';
});

function handleClose() {
	emit('close');
}

async function handleDownload() {
	if (!props.imageUrl) return;

	const a = document.createElement('a');
	a.download = props.filename || `image-${Date.now()}.png`;

	try {
		const response = await fetch(props.imageUrl);
		const blob = await response.blob();
		const url = URL.createObjectURL(blob);
		a.href = url;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);
		URL.revokeObjectURL(url);
	} catch (e) {
		a.href = props.imageUrl;
		a.target = '_blank';
		a.click();
	}
}

async function handleCopy() {
	if (!props.imageUrl) return;
	try {
		const response = await fetch(props.imageUrl);
		const blob = await response.blob();
		const imageBitmap = await createImageBitmap(blob);

		const canvas = document.createElement('canvas');
		canvas.width = imageBitmap.width;
		canvas.height = imageBitmap.height;
		const ctx = canvas.getContext('2d');
		if (!ctx) throw new Error('Failed to get canvas context');
		ctx.drawImage(imageBitmap, 0, 0);

		const pngBlob = await new Promise<Blob>((resolve, reject) => {
			canvas.toBlob(potentialBlob => {
				if (!potentialBlob) {
					reject(new Error('Failed to convert canvas to blob'));
					return;
				}
				resolve(potentialBlob);
			}, 'image/png');
		});

		await navigator.clipboard.write([
			new ClipboardItem({
				[pngBlob.type]: pngBlob,
			}),
		]);

		imageBitmap.close();
		copied.value = true;
		setTimeout(() => {
			copied.value = false;
		}, 2000);
	} catch (e) {
		console.error('Failed to copy image data:', e);
	}
}

watch(
	() => props.isOpen,
	newVal => {
		if (!newVal) {
			copied.value = false;
		}
	}
);
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
	transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
	opacity: 0;
}
</style>
