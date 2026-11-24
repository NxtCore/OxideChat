<template>
	<div class="space-y-3" data-testid="preset-knowledge">
		<div>
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p class="text-sm text-muted-foreground">{{ t('description') }}</p>
		</div>
		<p v-if="knowledgeStore.usable.length === 0" class="text-sm text-muted-foreground">{{ t('empty') }}</p>
		<label
			v-for="base in knowledgeStore.usable"
			:key="base.id"
			class="flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3 hover:bg-accent/30"
			:data-testid="`preset-knowledge-${base.name}`"
		>
			<ShadCheckbox class="mt-0.5" :model-value="form.knowledge_base_ids.includes(base.id)" @update:model-value="toggle(base.id)" />
			<span class="min-w-0">
				<span class="block text-sm font-medium text-foreground">{{ base.name }}</span>
				<span v-if="base.description" class="block truncate text-xs text-muted-foreground">{{ base.description }}</span>
			</span>
		</label>
		<div
			v-for="id in hidden"
			:key="id"
			class="flex items-center justify-between gap-3 rounded-lg border border-dashed border-border p-3 text-sm text-muted-foreground"
			data-testid="preset-knowledge-not-shared"
		>
			<span>{{ t('not_shared') }}</span>
			<ShadButton type="button" variant="ghost" size="sm" @click="toggle(id)">{{ t('remove') }}</ShadButton>
		</div>
		<p class="text-xs text-muted-foreground">{{ t('note') }}</p>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import {useKnowledgeStore} from '~/stores/knowledgeStore';
import {usePreferencesStore} from '~/stores/preferencesStore';
import {KNOWLEDGE_TOOL_NAME} from '~/composables/useBuiltinToolTemplates';

const props = defineProps<{form: {knowledge_base_ids: string[]; tool_ids: string[]}}>();
const preferencesStore = usePreferencesStore();
const store = useMainStore();
const knowledgeStore = useKnowledgeStore();
const t = (key: string) => store.getTranslation(`presets.editor.knowledge.${key}`);

const hidden = computed(() => (knowledgeStore.loaded ? props.form.knowledge_base_ids.filter(id => !knowledgeStore.usable.some(base => base.id === id)) : []));

function toggle(id: string) {
	const ids = props.form.knowledge_base_ids;
	props.form.knowledge_base_ids = ids.includes(id) ? ids.filter(selected => selected !== id) : [...ids, id];
	const tool = preferencesStore.userTools.find(tool => tool.source_kind === 'BUILTIN' && tool.name === KNOWLEDGE_TOOL_NAME);
	if (tool && props.form.knowledge_base_ids.includes(id) && !props.form.tool_ids.includes(tool.id)) props.form.tool_ids = [...props.form.tool_ids, tool.id];
}

onMounted(() => {
	knowledgeStore.ensure();
	preferencesStore.ensureTools();
});
</script>
