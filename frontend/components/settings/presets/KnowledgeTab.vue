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
		<p class="text-xs text-muted-foreground">{{ t('note') }}</p>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import {useKnowledgeStore} from '~/stores/knowledgeStore';

const props = defineProps<{form: {knowledge_base_ids: string[]}}>();
const store = useMainStore();
const knowledgeStore = useKnowledgeStore();
const t = (key: string) => store.getTranslation(`presets.editor.knowledge.${key}`);

function toggle(id: string) {
	const ids = props.form.knowledge_base_ids;
	props.form.knowledge_base_ids = ids.includes(id) ? ids.filter(selected => selected !== id) : [...ids, id];
}

onMounted(() => knowledgeStore.ensure());
</script>
