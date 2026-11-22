<template>
	<div class="space-y-4">
		<div>
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p class="text-sm text-muted-foreground">{{ t('description') }}</p>
		</div>
		<form class="flex gap-2" @submit.prevent="run">
			<ShadInput v-model="query" class="flex-1" :placeholder="t('placeholder')" maxlength="2000" data-testid="knowledge-search-query" />
			<ShadButton type="submit" :disabled="!query.trim() || searching" data-testid="knowledge-search-run">
				<Loader2 v-if="searching" class="h-4 w-4 animate-spin" />
				<Search v-else class="h-4 w-4" />
				{{ t('run') }}
			</ShadButton>
		</form>

		<p v-if="result?.error" class="text-sm text-destructive" data-testid="knowledge-search-error">{{ store.getTranslation(`rag.store_error.${result.error}`) }}</p>
		<p v-else-if="result && result.hits.length === 0" class="text-sm text-muted-foreground" data-testid="knowledge-search-empty">{{ t('empty') }}</p>

		<ol v-if="result?.hits.length" class="space-y-2">
			<li v-for="(hit, index) in result.hits" :key="index" class="rounded-lg border border-border bg-card p-3" data-testid="knowledge-search-hit">
				<div class="mb-1 flex flex-wrap items-center gap-2 text-sm">
					<span class="font-medium text-foreground">[{{ index + 1 }}]</span>
					<a v-if="hit.url" :href="hit.url" target="_blank" rel="noopener noreferrer" class="font-medium text-primary underline">{{ hit.title || hit.url }}</a>
					<span v-else-if="hit.title" class="font-medium text-foreground">{{ hit.title }}</span>
					<span v-if="hit.section" class="text-muted-foreground">› {{ hit.section }}</span>
					<span class="ml-auto font-mono text-xs text-muted-foreground" :title="t('score')">{{ hit.score.toFixed(3) }}</span>
				</div>
				<p class="line-clamp-4 whitespace-pre-line text-sm text-muted-foreground">{{ hit.text }}</p>
			</li>
		</ol>
	</div>
</template>

<script setup lang="ts">
import {Loader2, Search} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {KnowledgeSearchResponse} from '~/types/generated/KnowledgeSearchResponse';

const props = defineProps<{id: string}>();
const store = useMainStore();
const t = (key: string) => store.getTranslation(`knowledge.search.${key}`);
const query = ref('');
const searching = ref(false);
const result = ref<KnowledgeSearchResponse | null>(null);

async function run() {
	searching.value = true;
	try {
		result.value = await searchKnowledgeBase(props.id, query.value.trim());
	} catch (error: any) {
		store.toast(t('error'), {type: 'error', description: ragErrorMessage(error, '')});
	} finally {
		searching.value = false;
	}
}
</script>
