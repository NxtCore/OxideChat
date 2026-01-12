<template>
	<ShadDialog :open="isOpen" @update:open="handleOpenChange">
		<ShadDialogContent class="max-w-4xl h-[80vh] flex flex-col p-0 gap-0" hide-close>
			<ShadDialogHeader class="px-4 py-3 border-b border-border">
				<div class="flex items-center justify-between">
					<ShadDialogTitle class="text-sm font-medium">{{ store.getTranslation('chat.code_preview.title') }} {{ language.toUpperCase() }}</ShadDialogTitle>
					<div class="flex items-center gap-3">
						<ShadTabs v-model="activeTab">
							<ShadTabsList class="h-8">
								<ShadTabsTrigger value="preview" class="h-7 px-3 text-xs">{{ store.getTranslation('chat.code_preview.preview') }}</ShadTabsTrigger>
								<ShadTabsTrigger value="code" class="h-7 px-3 text-xs">{{ store.getTranslation('chat.code_preview.code') }}</ShadTabsTrigger>
							</ShadTabsList>
						</ShadTabs>

						<ShadDialogClose as-child>
							<ShadButton variant="ghost" size="icon" class="h-8 w-8">
								<X class="size-4" />
							</ShadButton>
						</ShadDialogClose>
					</div>
				</div>
			</ShadDialogHeader>

			<div class="flex-1 min-h-0 overflow-hidden">
				<div v-show="activeTab === 'preview'" class="h-full bg-white">
					<iframe ref="previewFrame" :key="sandbox" class="w-full h-full border-0" :srcdoc="previewHtml" :sandbox="sandbox" />
				</div>

				<div v-show="activeTab === 'code'" class="h-full overflow-auto p-4 bg-muted/30">
					<div class="rounded-lg border border-border bg-card overflow-hidden">
						<div class="flex items-center justify-between px-4 py-2 bg-muted border-b border-border">
							<span class="text-xs font-mono text-muted-foreground uppercase">{{ language }}</span>
							<ShadButton variant="ghost" size="sm" class="h-7 text-xs" @click="copyCode">
								<Check v-if="copied" class="size-3.5 mr-1" />
								<Copy v-else class="size-3.5 mr-1" />
								{{ copied ? 'Copied' : 'Copy' }}
							</ShadButton>
						</div>
						<pre class="p-4 overflow-auto"><code class="text-sm font-mono leading-relaxed">{{ code }}</code></pre>
					</div>
				</div>
			</div>

			<ShadDialogFooter class="px-4 py-3 border-t border-border">
				<div class="flex items-center justify-between w-full">
					<div class="flex items-center gap-4">
						<span class="text-xs text-muted-foreground"> {{ code.split('\n').length }} lines </span>
						<div class="flex items-center gap-2">
							<ShadSwitch id="sandbox-toggle" v-model:modelValue="allowUnrestricted" />
							<ShadLabel for="sandbox-toggle" class="text-[10px] uppercase font-bold text-muted-foreground cursor-pointer">Unrestricted</ShadLabel>
						</div>
					</div>
					<div class="flex gap-2">
						<ShadButton variant="outline" size="sm" @click="openInNewTab"> Open in Tab </ShadButton>
						<ShadDialogClose as-child>
							<ShadButton size="sm">Close</ShadButton>
						</ShadDialogClose>
					</div>
				</div>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Copy, Check, X} from 'lucide-vue-next';
import {generatePreviewHtml} from '~/composables/useMarkdown';
import {useMainStore} from '~/stores';

const store = useMainStore();

const props = defineProps<{
	code: string;
	language: string;
	isOpen: boolean;
}>();

const emit = defineEmits<{
	(e: 'close'): void;
}>();

const activeTab = ref<'preview' | 'code'>('preview');
const copied = ref(false);
const allowUnrestricted = ref(false);

const sandbox = computed(() => {
	if (allowUnrestricted.value) {
		return 'allow-scripts allow-forms allow-modals allow-popups allow-same-origin';
	}
	return 'allow-scripts';
});

const previewHtml = computed(() => {
	return generatePreviewHtml(props.code, props.language);
});

function handleOpenChange(open: boolean) {
	if (!open) {
		emit('close');
	}
}

function copyCode() {
	navigator.clipboard.writeText(props.code);
	copied.value = true;
	setTimeout(() => {
		copied.value = false;
	}, 2000);
}

function openInNewTab() {
	const win = window.open('', '_blank');
	if (win) {
		win.document.write(previewHtml.value);
		win.document.close();
	}
}
</script>
