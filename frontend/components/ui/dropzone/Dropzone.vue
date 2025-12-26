<script setup>
import {ref, computed} from 'vue';
import {cn} from '@/lib/utils';
import {Upload, X, File, Image, FileText, FileAudio, FileVideo} from 'lucide-vue-next';

const props = defineProps({
	accept: {
		type: [String, Array],
		default: '*/*',
	},
	multiple: {
		type: Boolean,
		default: true,
	},
	maxFiles: {
		type: Number,
		default: 10,
	},
	maxSize: {
		type: Number,
		default: 5242880, // 5MB
	},
	disabled: {
		type: Boolean,
		default: false,
	},
	class: {
		type: String,
		default: '',
	},
	title: {
		type: String,
		default: 'Drop files here or click to browse',
	},
	description: {
		type: String,
		default: 'Supports multiple files, max 5MB per file',
	},
});

const emit = defineEmits(['files-selected', 'files-rejected', 'file-removed']);

const isDragOver = ref(false);
const files = ref([]);
const fileInputRef = ref(null);

const formatFileSize = bytes => {
	if (bytes === 0) return '0 Bytes';
	const k = 1024;
	const sizes = ['Bytes', 'KB', 'MB', 'GB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

const getFileIcon = file => {
	const type = file.type;
	if (type.startsWith('image/')) return Image;
	if (type.startsWith('video/')) return FileVideo;
	if (type.startsWith('audio/')) return FileAudio;
	if (type.includes('text') || type.includes('json') || type.includes('csv')) return FileText;
	return File;
};

const validateFiles = fileList => {
	const validFiles = [];
	const rejectedFiles = [];

	Array.from(fileList).forEach(file => {
		// Check file type
		let isValidType = false;
		if (Array.isArray(props.accept)) {
			isValidType = props.accept.includes(file.type);
		} else if (props.accept === '*/*' || props.accept === '') {
			isValidType = true;
		} else {
			// Handle wildcards like 'image/*'
			const acceptTypes = props.accept.split(',').map(t => t.trim());
			isValidType = acceptTypes.some(acceptType => {
				if (acceptType === '*/*') return true;
				if (acceptType.endsWith('/*')) {
					const category = acceptType.split('/')[0];
					return file.type.startsWith(category + '/');
				}
				return file.type === acceptType;
			});
		}

		const isValidSize = file.size <= props.maxSize;
		const isValidCount = files.value.length + validFiles.length < props.maxFiles;

		if (isValidType && isValidSize && isValidCount) {
			validFiles.push(file);
		} else {
			const reason = !isValidType ? 'Invalid file type' : !isValidSize ? `File too large (max ${formatFileSize(props.maxSize)})` : 'Too many files';
			rejectedFiles.push({file, reason});
		}
	});

	return {validFiles, rejectedFiles};
};

const handleFiles = fileList => {
	if (props.disabled) return;

	const {validFiles, rejectedFiles} = validateFiles(fileList);

	if (!props.multiple && validFiles.length > 0) {
		// For single file mode, clear existing files
		files.value = [];
	}

	validFiles.forEach(file => {
		files.value.push({
			id: Date.now() + Math.random(),
			file,
			name: file.name,
			size: file.size,
			type: file.type,
			preview: file.type.startsWith('image/') ? URL.createObjectURL(file) : null,
		});
	});

	if (validFiles.length > 0) {
		emit('files-selected', validFiles);
	}

	if (rejectedFiles.length > 0) {
		emit('files-rejected', rejectedFiles);
	}
};

const handleDragOver = e => {
	e.preventDefault();
	if (!props.disabled) {
		isDragOver.value = true;
	}
};

const handleDragLeave = e => {
	e.preventDefault();
	isDragOver.value = false;
};

const handleDrop = e => {
	e.preventDefault();
	isDragOver.value = false;

	if (!props.disabled && e.dataTransfer?.files) {
		handleFiles(e.dataTransfer.files);
	}
};

const handleFileSelect = e => {
	if (e.target?.files && e.target.files.length > 0) {
		handleFiles(e.target.files);
		// Reset input to allow selecting the same file again
		if (fileInputRef.value) {
			fileInputRef.value.value = '';
		}
	}
};

const removeFile = index => {
	const file = files.value[index];
	if (file.preview) {
		URL.revokeObjectURL(file.preview);
	}
	files.value.splice(index, 1);
	emit('file-removed', file);
};

const clearAllFiles = () => {
	files.value.forEach(file => {
		if (file.preview) {
			URL.revokeObjectURL(file.preview);
		}
	});
	files.value = [];
};

const isDragActive = computed(() => isDragOver.value && !props.disabled);

defineExpose({
	files: files.value,
	clearAllFiles,
});
</script>

<template>
	<div
		:class="
			cn(
				'relative flex flex-col gap-4 rounded-lg border-2 border-dashed transition-colors',
				isDragActive ? 'border-primary bg-primary/5' : 'border-border',
				props.disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer',
				props.class
			)
		"
		@dragover="handleDragOver"
		@dragleave="handleDragLeave"
		@drop="handleDrop"
	>
		<input
			ref="fileInputRef"
			:accept="Array.isArray(accept) ? accept.join(',') : accept"
			:multiple="multiple"
			:disabled="disabled"
			type="file"
			class="absolute inset-0 h-full w-full cursor-pointer opacity-0 z-10"
			@change="handleFileSelect"
			@click="e => e.stopPropagation()"
		/>

		<div :class="cn('flex flex-col items-center justify-center gap-2 p-8 text-center', isDragActive ? 'text-primary' : 'text-muted-foreground')">
			<Upload :class="cn('h-8 w-8', isDragActive ? 'text-primary' : 'text-muted-foreground')" />
			<div>
				<h3 class="text-sm font-medium">
					{{ props.title }}
				</h3>
				<p class="text-xs text-muted-foreground mt-1">
					{{ props.description }}
				</p>
			</div>
			<div class="flex gap-2 text-xs text-muted-foreground">
				<span v-if="typeof accept === 'string' && accept !== '*/*'">{{ accept }}</span>
				<span v-else>Any file type</span>
				<span>•</span>
				<span>Max {{ formatFileSize(maxSize) }}</span>
				<span>•</span>
				<span>{{ maxFiles }} files max</span>
			</div>
		</div>

		<!-- Selected Files -->
		<div v-if="files.length > 0" class="px-4 pb-4">
			<div class="flex items-center justify-between mb-3">
				<h4 class="text-sm font-medium">Selected Files ({{ files.length }}/{{ maxFiles }})</h4>
				<button v-if="files.length > 0" @click.stop="clearAllFiles" class="text-xs text-muted-foreground hover:text-foreground transition-colors z-10 relative">
					Clear all
				</button>
			</div>

			<div class="space-y-2 max-h-48 overflow-y-auto">
				<div
					v-for="(file, index) in files"
					:key="file.id"
					:class="cn('flex items-center gap-3 p-3 rounded-lg border bg-card', 'transition-colors hover:bg-accent')"
				>
					<!-- File Preview -->
					<div v-if="file.preview" class="flex-shrink-0">
						<img :src="file.preview" :alt="file.name" class="h-10 w-10 rounded object-cover" />
					</div>
					<div v-else class="flex-shrink-0">
						<component :is="getFileIcon(file)" class="h-10 w-10 text-muted-foreground" />
					</div>

					<!-- File Info -->
					<div class="flex-1 min-w-0">
						<p class="text-sm font-medium truncate">{{ file.name }}</p>
						<p class="text-xs text-muted-foreground">{{ formatFileSize(file.size) }}</p>
					</div>

					<!-- Remove Button -->
					<button
						@click.stop="removeFile(index)"
						:disabled="disabled"
						class="flex-shrink-0 p-1 rounded-md hover:bg-accent transition-colors disabled:cursor-not-allowed z-10 relative"
					>
						<X class="h-4 w-4" />
					</button>
				</div>
			</div>
		</div>
	</div>
</template>
