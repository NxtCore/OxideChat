<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template v-if="canManage" #actions>
			<ShadButton data-testid="knowledge-create" @click="router.push('/settings/knowledge/new')">
				<Plus class="h-4 w-4" />
				{{ t('create') }}
			</ShadButton>
		</template>

		<div v-if="bases === null" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="bases.length === 0" class="flex flex-col items-center rounded-xl border border-dashed border-border px-6 py-12 text-center" data-testid="knowledge-empty">
			<BookOpen class="mb-4 h-10 w-10 text-primary" />
			<p class="font-medium text-foreground">{{ t('empty_title') }}</p>
			<p class="mt-1 max-w-md text-sm text-muted-foreground">{{ t(canManage ? 'empty_admin' : 'empty_user') }}</p>
		</div>

		<div v-else class="space-y-2">
			<NuxtLink
				v-for="base in bases"
				:key="base.id"
				:to="`/settings/knowledge/${base.id}`"
				class="flex items-center gap-4 rounded-lg border border-border bg-card px-4 py-3 transition-colors hover:bg-accent/40"
				:data-testid="`knowledge-row-${base.name}`"
			>
				<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-primary/10">
					<BookOpen class="h-5 w-5 text-primary" />
				</div>
				<div class="min-w-0 flex-1">
					<p class="font-medium text-foreground">{{ base.name }}</p>
					<p class="truncate text-sm text-muted-foreground">{{ base.description || (base.connection ? `${base.connection.name} · ${base.view?.collection}` : '') }}</p>
				</div>
			</NuxtLink>
		</div>
	</SettingsPage>
</template>

<script setup lang="ts">
import {BookOpen, Loader2, Plus} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_RAG_EDIT} from '~/types/generated/permissions';
import {useKnowledgeStore} from '~/stores/knowledgeStore';

const store = useMainStore();
const router = useRouter();
const t = (key: string) => store.getTranslation(`knowledge.list.${key}`);
const canManage = computed(() => store.hasPermission(ADMIN_RAG_EDIT));
const knowledgeStore = useKnowledgeStore();
const failed = ref(false);
const bases = computed(() => (knowledgeStore.loaded ? knowledgeStore.bases : failed.value ? [] : null));

onMounted(async () => {
	await knowledgeStore.ensure();
	if (!knowledgeStore.loaded) {
		failed.value = true;
		store.toast(t('load_error'), {type: 'error'});
	}
});
</script>
