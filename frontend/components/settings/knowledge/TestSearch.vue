<template>
	<div class="space-y-4">
		<div>
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p class="text-sm text-muted-foreground">{{ t('description') }}</p>
		</div>
		<div v-if="canSearchAs" class="space-y-1">
			<ShadLabel>{{ t('as') }}</ShadLabel>
			<div class="flex flex-wrap items-center gap-2">
				<ShadPopover v-model:open="pickerOpen">
					<ShadPopoverTrigger as-child>
						<ShadButton variant="outline" class="min-w-48 justify-between" data-testid="knowledge-search-as">
							{{ asUser?.label ?? t('as_me') }}
							<ChevronsUpDown class="h-4 w-4 opacity-50" />
						</ShadButton>
					</ShadPopoverTrigger>
					<ShadPopoverContent class="w-72 space-y-2 p-2" align="start">
						<ShadInput v-model="userSearch" class="h-8 text-sm" :placeholder="store.getTranslation('assignments.search')" data-testid="knowledge-search-as-search" />
						<ul class="max-h-64 overflow-auto">
							<li>
								<button type="button" class="w-full rounded px-2 py-1.5 text-left text-sm hover:bg-muted" @click="pickUser(null)">{{ t('as_me') }}</button>
							</li>
							<li v-for="user in users" :key="user.id ?? ''">
								<button type="button" class="w-full truncate rounded px-2 py-1.5 text-left text-sm hover:bg-muted" :data-testid="`knowledge-search-as-user-${user.id}`" @click="pickUser(user)">
									{{ user.label }}
								</button>
							</li>
						</ul>
					</ShadPopoverContent>
				</ShadPopover>
				<span class="text-xs text-muted-foreground">{{ t('as_hint') }}</span>
			</div>
		</div>
		<form class="flex gap-2" @submit.prevent="run">
			<ShadInput v-model="query" class="flex-1" :placeholder="t('placeholder')" maxlength="2000" data-testid="knowledge-search-query" />
			<ShadButton type="submit" :disabled="!query.trim() || searching" data-testid="knowledge-search-run">
				<Loader2 v-if="searching" class="h-4 w-4 animate-spin" />
				<Search v-else class="h-4 w-4" />
				{{ t('run') }}
			</ShadButton>
		</form>

		<p v-if="explanation" class="text-sm text-muted-foreground" data-testid="knowledge-search-explanation">{{ explanation }}</p>
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
import {ChevronsUpDown, Loader2, Search} from 'lucide-vue-next';
import type {AssignmentOption} from '~/types/assignments';
import {useMainStore} from '~/stores';
import type {KnowledgeSearchResponse} from '~/types/generated/KnowledgeSearchResponse';

const props = defineProps<{id: string; canSearchAs?: boolean}>();
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.search.${key}`, params);
const {users, searchUsers} = useAssignmentOptions();
const pickerOpen = ref(false);
const userSearch = ref('');
const asUser = ref<AssignmentOption | null>(null);
const searchedAs = ref<AssignmentOption | null>(null);
const query = ref('');
const searching = ref(false);
const result = ref<KnowledgeSearchResponse | null>(null);

async function run() {
	searching.value = true;
	try {
		result.value = await searchKnowledgeBase(props.id, query.value.trim(), asUser.value?.id);
		searchedAs.value = asUser.value;
	} catch (error: any) {
		store.toast(t('error'), {type: 'error', description: ragErrorMessage(error, '')});
	} finally {
		searching.value = false;
	}
}

const explanation = computed(() => {
	const access = result.value?.access;
	const name = searchedAs.value?.label ?? '';
	if (!access) return '';
	if (!access.usable) return t('explain_not_shared', {name});
	if (!access.per_record) return t('explain_all', {name});
	const labels = String(access.labels);
	const held = access.held === 0 ? t('explain_none', {name, labels}) : t('explain_held', {name, held: String(access.held), labels});
	return access.unassigned ? `${held} ${t('explain_unassigned', {count: String(access.unassigned)})}` : held;
});

function pickUser(user: AssignmentOption | null) {
	asUser.value = user;
	pickerOpen.value = false;
}

watch(userSearch, search => searchUsers(search.trim()));

onMounted(() => {
	if (props.canSearchAs) searchUsers();
});
</script>
