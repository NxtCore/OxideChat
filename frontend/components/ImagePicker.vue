<template>
	<div class="flex items-center gap-3">
		<button
			type="button"
			:data-testid="testId"
			:disabled="disabled"
			:aria-label="t('change')"
			:class="
				cn(
					'group relative flex shrink-0 items-center justify-center overflow-hidden rounded-lg border bg-muted outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60',
					dragging ? 'border-primary ring-2 ring-primary/30' : 'border-border',
					sizeClass,
				)
			"
			@click="open = true"
			@dragenter.prevent="onDragEnter"
			@dragover.prevent
			@dragleave="onDragLeave"
			@drop.prevent="onDrop"
			@paste="onPaste"
		>
			<img v-if="src && !broken" :src="src" alt="" class="h-full w-full object-cover" :data-testid="`${testId}-preview`" @error="broken = true" />
			<slot v-else name="fallback">
				<ImageIcon class="h-1/2 w-1/2 text-muted-foreground" />
			</slot>
			<span
				v-if="!disabled"
				class="absolute inset-0 flex items-center justify-center bg-background/70 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
				:class="{'opacity-100': uploading || dragging}"
			>
				<Loader2 v-if="uploading" class="h-5 w-5 animate-spin text-foreground" />
				<Upload v-else-if="dragging" class="h-5 w-5 text-primary" />
				<Pencil v-else class="h-4 w-4 text-foreground" />
			</span>
		</button>

		<div :class="cn('flex min-w-0 flex-col justify-center gap-1.5', heightClass)">
			<div class="flex flex-wrap gap-2">
				<ShadButton variant="outline" size="sm" type="button" class="h-8 gap-1.5" :disabled="disabled || uploading" :data-testid="`${testId}-change`" @click="open = true">
					<Upload class="h-3.5 w-3.5" />
					{{ modelValue ? t('change') : t('choose') }}
				</ShadButton>
				<ShadButton v-if="modelValue" variant="ghost" size="sm" type="button" class="h-8 gap-1.5 text-destructive" :disabled="disabled" :data-testid="`${testId}-remove`" @click="setValue(null)">
					<Trash2 class="h-3.5 w-3.5" />
					{{ store.getTranslation('common.remove') }}
				</ShadButton>
			</div>
			<p v-if="broken && src" class="flex items-center gap-1 text-xs text-destructive">
				<AlertTriangle class="h-3 w-3 shrink-0" />
				{{ t('broken') }}
			</p>
			<p v-else class="truncate text-xs text-muted-foreground">{{ t('drop_hint') }}</p>
		</div>

		<input ref="fileInput" type="file" :accept="IMAGE_UPLOAD_TYPES.join(',')" class="hidden" :data-testid="`${testId}-input`" @change="onFileInput" />

		<ShadDialog v-model:open="open">
			<ShadDialogContent class="sm:max-w-md" :data-testid="`${testId}-dialog`" @paste="onPaste">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ title ?? t('title') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('description') }}</ShadDialogDescription>
				</ShadDialogHeader>

				<button
					type="button"
					:data-testid="`${testId}-dropzone`"
					:disabled="uploading"
					:class="
						cn(
							'flex w-full flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-6 py-8 text-center transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring',
							dragging ? 'border-primary bg-primary/5' : 'border-border hover:border-primary/50 hover:bg-accent/30',
						)
					"
					@click="fileInput?.click()"
					@dragenter.prevent="onDragEnter"
					@dragover.prevent
					@dragleave="onDragLeave"
					@drop.prevent="onDrop"
				>
					<div class="flex h-20 w-20 items-center justify-center overflow-hidden rounded-xl border border-border bg-muted">
						<Loader2 v-if="uploading" class="h-6 w-6 animate-spin text-muted-foreground" />
						<img v-else-if="src && !broken" :src="src" alt="" class="h-full w-full object-cover" />
						<slot v-else name="fallback">
							<ImageIcon class="h-8 w-8 text-muted-foreground" />
						</slot>
					</div>
					<div class="space-y-1">
						<p class="text-sm font-medium text-foreground">{{ uploading ? t('uploading') : dragging ? t('drop_now') : t('drop_title') }}</p>
						<p class="text-xs text-muted-foreground">{{ t('formats') }}</p>
					</div>
				</button>

				<template v-if="allowLink">
					<div class="flex items-center gap-3 text-xs text-muted-foreground">
						<div class="h-px flex-1 bg-border" />
						{{ t('or_link') }}
						<div class="h-px flex-1 bg-border" />
					</div>
					<form class="space-y-1.5" @submit.prevent="applyLink">
						<div class="flex gap-2">
							<ShadInput v-model="link" inputmode="url" placeholder="https://example.com/icon.png" maxlength="500" class="flex-1" :data-testid="`${testId}-link`" />
							<ShadButton type="submit" variant="outline" :disabled="!link.trim()" :data-testid="`${testId}-link-apply`">{{ t('use_link') }}</ShadButton>
						</div>
						<p v-if="linkError" class="text-xs text-destructive">{{ t(linkError) }}</p>
					</form>
				</template>

				<ShadDialogFooter class="sm:justify-between">
					<ShadButton v-if="modelValue" variant="ghost" type="button" class="text-destructive" @click="removeAndClose">
						<Trash2 class="h-4 w-4" />
						{{ store.getTranslation('common.remove') }}
					</ShadButton>
					<span v-else />
					<ShadButton variant="outline" type="button" @click="open = false">{{ t('close') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, ImageIcon, Loader2, Pencil, Trash2, Upload} from 'lucide-vue-next';
import {cn} from '~/lib/utils';
import {useMainStore} from '~/stores';

const props = withDefaults(
	defineProps<{
		modelValue: string | null;
		valueMode?: 'url' | 'id';
		allowLink?: boolean;
		disabled?: boolean;
		size?: 'sm' | 'md';
		title?: string;
		testId?: string;
	}>(),
	{valueMode: 'url', allowLink: true, disabled: false, size: 'md', title: undefined, testId: 'image-picker'},
);

const emit = defineEmits<{'update:modelValue': [value: string | null]}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`image_picker.${key}`);
const {uploading, upload} = useImageUpload();

const open = ref(false);
const dragging = ref(false);
const broken = ref(false);
const link = ref('');
const linkError = ref<string | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
let dragDepth = 0;

const heightClass = computed(() => (props.size === 'sm' ? 'h-12' : 'h-16'));
const sizeClass = computed(() => (props.size === 'sm' ? 'h-12 w-12' : 'h-16 w-16'));
const src = computed(() => (props.modelValue ? (props.valueMode === 'id' ? imageUrl(props.modelValue) : props.modelValue) : null));

watch(src, () => (broken.value = false));
watch(open, isOpen => {
	if (!isOpen) return;
	link.value = props.valueMode === 'url' && props.modelValue && !props.modelValue.startsWith('/api/v1/images/') ? props.modelValue : '';
	linkError.value = null;
});

function setValue(value: string | null) {
	emit('update:modelValue', value);
}

async function uploadFile(file: File) {
	if (props.disabled || uploading.value) return;
	try {
		const stored = await upload(file);
		setValue(props.valueMode === 'id' ? stored.id : stored.url);
		open.value = false;
	} catch (error) {
		store.toast(t(error instanceof ImageUploadError ? error.key.replace('image_picker.', '') : 'errors.upload'), {type: 'error'});
	}
}

function onFileInput(event: Event) {
	const target = event.target as HTMLInputElement;
	const file = firstImageFile(target.files);
	target.value = '';
	if (file) uploadFile(file);
}

function onDragEnter(event: DragEvent) {
	if (props.disabled || !event.dataTransfer?.types.includes('Files')) return;
	dragDepth++;
	dragging.value = true;
}

function onDragLeave() {
	dragDepth = Math.max(0, dragDepth - 1);
	if (dragDepth === 0) dragging.value = false;
}

function onDrop(event: DragEvent) {
	dragDepth = 0;
	dragging.value = false;
	const file = firstImageFile(event.dataTransfer?.files);
	if (file) uploadFile(file);
}

function onPaste(event: ClipboardEvent) {
	const file = firstImageFile(event.clipboardData?.items);
	if (!file) return;
	event.preventDefault();
	uploadFile(file);
}

function applyLink() {
	const value = link.value.trim();
	if (!/^https?:\/\/\S+$/i.test(value)) {
		linkError.value = 'errors.link';
		return;
	}
	setValue(value);
	open.value = false;
}

function removeAndClose() {
	setValue(null);
	open.value = false;
}
</script>
