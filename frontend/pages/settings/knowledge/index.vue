<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template v-if="canCreate" #actions>
			<ShadButton data-testid="knowledge-create" @click="router.push('/settings/knowledge/new')">
				<Plus class="h-4 w-4" />
				{{ t('create') }}
			</ShadButton>
		</template>

		<ShadTabs v-if="bases !== null && bases.length > 0" v-model="activeTab" class="mb-4 w-full">
			<ShadTabsList>
				<ShadTabsTrigger value="mine" data-testid="knowledge-list-tab-mine">{{ t('tabs.mine') }}</ShadTabsTrigger>
				<ShadTabsTrigger value="shared" data-testid="knowledge-list-tab-shared">{{ t('tabs.shared') }}</ShadTabsTrigger>
				<ShadTabsTrigger v-if="canViewAll" value="all" data-testid="knowledge-list-tab-all">{{ t('tabs.all') }}</ShadTabsTrigger>
			</ShadTabsList>
		</ShadTabs>

		<div v-if="bases === null" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="visibleBases.length === 0" class="flex flex-col items-center rounded-xl border border-dashed border-border px-6 py-12 text-center" data-testid="knowledge-empty">
			<BookOpen class="mb-4 h-10 w-10 text-primary" />
			<p class="font-medium text-foreground">{{ t('empty_title') }}</p>
			<p class="mt-1 max-w-md text-sm text-muted-foreground">{{ t(canCreate ? 'empty_admin' : 'empty_user') }}</p>
		</div>

		<div v-else class="space-y-2">
			<NuxtLink
				v-for="base in visibleBases"
				:key="base.id"
				:to="`/settings/knowledge/${base.id}`"
				class="flex items-center gap-4 rounded-lg border border-border bg-card px-4 py-3 transition-colors hover:bg-accent/40"
				:data-testid="`knowledge-row-${base.name}`"
			>
				<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-primary/10">
					<BookOpen class="h-5 w-5 text-primary" />
				</div>
				<div class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<p class="font-medium text-foreground">{{ base.name }}</p>
						<span class="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
							{{ t(base.mode === 'managed' ? 'mode_managed' : 'mode_external') }}
						</span>
						<span v-if="base.mode === 'managed'" class="rounded-full border border-border px-2 py-0.5 text-xs text-muted-foreground">
							{{ base.team_name || t(`scope_${base.scope}`) }}
						</span>
					</div>
					<p class="truncate text-sm text-muted-foreground">
						{{ subtitle(base) }}
					</p>
				</div>
			</NuxtLink>
		</div>
	</SettingsPage>
</template>

<script setup lang="ts">
import {BookOpen, Loader2, Plus} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_RAG_EDIT, ADMIN_RAG_VIEW} from '~/types/generated/permissions';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import type {KnowledgeScopesResponse} from '~/types/generated/KnowledgeScopesResponse';
import {useKnowledgeStore} from '~/stores/knowledgeStore';

const store = useMainStore();
const router = useRouter();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.list.${key}`, params);
const canViewAll = computed(() => store.hasPermission(ADMIN_RAG_VIEW) || store.hasPermission(ADMIN_RAG_EDIT));
const knowledgeStore = useKnowledgeStore();
const scopes = ref<KnowledgeScopesResponse | null>(null);
const failed = ref(false);
const bases = computed(() => (knowledgeStore.loaded ? knowledgeStore.bases : failed.value ? [] : null));

const canCreate = computed(() => {
	if (!scopes.value) return store.hasPermission(ADMIN_RAG_EDIT);
	const canManaged = scopes.value.managed_available && (scopes.value.personal || scopes.value.teams.length > 0 || scopes.value.installation);
	return scopes.value.external || canManaged;
});

const mineBases = computed(() => (bases.value ?? []).filter(base => base.owned));
const sharedBases = computed(() => (bases.value ?? []).filter(base => base.usable && !base.owned));
const activeTab = ref<'mine' | 'shared' | 'all'>(canViewAll.value ? 'all' : 'mine');

const visibleBases = computed(() => {
	if (!bases.value) return [];
	if (activeTab.value === 'all') return bases.value;
	if (activeTab.value === 'mine') return mineBases.value;
	return sharedBases.value;
});

function subtitle(base: KnowledgeBaseResponse): string {
	if (base.description) return base.description;
	if (base.mode === 'managed') return t('documents_count', {count: String(base.document_count)});
	return base.connection ? `${base.connection.name} · ${base.view?.collection}` : '';
}

onMounted(async () => {
	const [scopeResult] = await Promise.all([knowledgeScopes().catch(() => null), knowledgeStore.ensure()]);
	scopes.value = scopeResult;
	if (!knowledgeStore.loaded) {
		failed.value = true;
		store.toast(t('load_error'), {type: 'error'});
		return;
	}
	if (!canViewAll.value) {
		activeTab.value = mineBases.value.length > 0 ? 'mine' : sharedBases.value.length > 0 ? 'shared' : 'mine';
	}
});
</script>
