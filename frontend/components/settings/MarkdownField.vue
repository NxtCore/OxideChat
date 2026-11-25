<template>
	<ShadTabs v-model="tab" class="space-y-1.5">
		<ShadTabsList class="h-8">
			<ShadTabsTrigger value="write" class="h-7 px-3 text-xs">{{ t('description_tab_write') }}</ShadTabsTrigger>
			<ShadTabsTrigger value="preview" class="h-7 px-3 text-xs" data-testid="markdown-preview-tab">{{ t('description_tab_preview') }}</ShadTabsTrigger>
		</ShadTabsList>
		<ShadTextarea v-if="tab === 'write'" :id="id" v-model="value" :placeholder="placeholder" :maxlength="maxlength" :readonly="readonly" rows="5" />
		<div v-else class="min-h-[7.5rem] rounded-md border border-input px-3 py-2" data-testid="markdown-preview">
			<div v-if="value?.trim()" class="prose prose-sm dark:prose-invert max-w-none" v-html="renderComplete(value ?? '')" />
			<p v-else class="text-sm text-muted-foreground">{{ t('description_empty_preview') }}</p>
		</div>
		<p class="text-xs text-muted-foreground">{{ t('description_markdown_hint') }}</p>
	</ShadTabs>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';

defineProps<{
	id?: string;
	placeholder?: string;
	maxlength?: number;
	readonly?: boolean;
}>();

const value = defineModel<string | null>({default: ''});

const store = useMainStore();
const {renderComplete} = useMarkdown();
const t = (key: string) => store.getTranslation(`settings.models.editor.${key}`);

const tab = ref('write');
</script>
