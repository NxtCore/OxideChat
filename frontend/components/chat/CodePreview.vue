<template>
	<Dialog :open="isOpen" @update:open="handleOpenChange">
		<DialogContent class="max-w-4xl h-[80vh] flex flex-col p-0 gap-0">
			<DialogHeader class="px-4 py-3 border-b border-border">
				<div class="flex items-center justify-between">
					<DialogTitle class="text-sm font-medium"> Preview: {{ language.toUpperCase() }} </DialogTitle>
					<Tabs v-model="activeTab">
						<TabsList class="h-8">
							<TabsTrigger value="preview" class="h-7 px-3 text-xs"> Preview </TabsTrigger>
							<TabsTrigger value="code" class="h-7 px-3 text-xs"> Code </TabsTrigger>
						</TabsList>
					</Tabs>
				</div>
			</DialogHeader>

			<div class="flex-1 min-h-0 overflow-hidden">
				<div v-show="activeTab === 'preview'" class="h-full bg-white">
					<iframe ref="previewFrame" class="w-full h-full border-0" :srcdoc="previewHtml" sandbox="allow-scripts" />
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

			<DialogFooter class="px-4 py-3 border-t border-border">
				<div class="flex items-center justify-between w-full">
					<span class="text-xs text-muted-foreground"> {{ code.split('\n').length }} lines </span>
					<div class="flex gap-2">
						<ShadButton variant="outline" size="sm" @click="openInNewTab"> Open in Tab </ShadButton>
						<DialogClose as-child>
							<ShadButton size="sm">Close</ShadButton>
						</DialogClose>
					</div>
				</div>
			</DialogFooter>
		</DialogContent>
	</Dialog>
</template>

<script setup lang="ts">
import {Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogClose} from '@/components/ui/dialog';
import {Tabs, TabsList, TabsTrigger} from '@/components/ui/tabs';
import {Button as ShadButton} from '@/components/ui/button';
import {Copy, Check} from 'lucide-vue-next';
import {generatePreviewHtml} from '~/composables/useMarkdown';

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
const previewFrame = ref<HTMLIFrameElement | null>(null);

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
