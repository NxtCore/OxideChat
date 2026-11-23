<template>
	<SettingsPage :title="pageTitle" :description="t('description')" :form="settingsForm">
		<template #leading>
			<ShadButton variant="ghost" size="icon" class="shrink-0" data-testid="knowledge-editor-back" @click="router.push('/settings/knowledge')">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>
		</template>
		<template v-if="!isNew && base?.can_manage" #actions>
			<ShadButton variant="outline" data-testid="knowledge-delete" @click="deleteOpen = true">
				<Trash2 class="h-4 w-4" />
				{{ store.getTranslation('common.delete') }}
			</ShadButton>
		</template>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<p v-else-if="notFound" class="py-12 text-center text-muted-foreground">{{ t('not_found') }}</p>

		<template v-else>
			<ShadTabs v-model="currentTab" class="mb-5 w-full">
				<ShadTabsList>
					<ShadTabsTrigger v-for="tab in tabs" :key="tab" :value="tab" :data-testid="`knowledge-tab-${tab}`">{{ t(`tabs.${tab}`) }}</ShadTabsTrigger>
				</ShadTabsList>
			</ShadTabs>

			<fieldset v-show="currentTab === 'general'" :disabled="!canEdit" class="space-y-4">
				<div class="space-y-2">
					<ShadLabel for="knowledge-name">{{ t('name') }}</ShadLabel>
					<ShadInput id="knowledge-name" v-model="form.name" maxlength="100" data-testid="knowledge-name" />
				</div>
				<div class="space-y-2">
					<ShadLabel for="knowledge-description">{{ t('description_label') }}</ShadLabel>
					<SettingsMarkdownField id="knowledge-description" v-model="form.description" :maxlength="2000" />
					<p class="text-xs text-muted-foreground">{{ t('description_hint') }}</p>
				</div>
			</fieldset>

			<SettingsKnowledgeSourceForm v-if="canManage && sourceOpened" v-show="currentTab === 'source'" v-model="form.view" />

			<SettingsKnowledgeSharing v-if="!isNew && canManage && currentTab === 'sharing'" :id="id" :per-record="!!base?.view?.access" />

			<SettingsKnowledgeAccessLabels v-if="!isNew && canManage && currentTab === 'access'" :id="id" :can-manage="canManage" />

			<SettingsKnowledgeModelAccess v-if="!isNew && canEdit && currentTab === 'models'" :id="id" />

			<SettingsKnowledgeTestSearch v-if="!isNew && canSearch && currentTab === 'search'" :id="id" :can-search-as="canManage" />
		</template>

		<ShadDialog v-model:open="deleteOpen">
			<ShadDialogContent>
				<ShadDialogHeader>
					<ShadDialogTitle>{{ t('delete_title') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('delete_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="deleteOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" data-testid="knowledge-delete-confirm" @click="remove">{{ store.getTranslation('common.delete') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ArrowLeft, Loader2, Trash2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_RAG_EDIT} from '~/types/generated/permissions';
import type {ExternalView} from '~/types/generated/ExternalView';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import {emptyView} from '~/utils/knowledge';

const store = useMainStore();
const route = useRoute();
const router = useRouter();
const t = (key: string) => store.getTranslation(`knowledge.editor.${key}`);

const id = String(route.params.id);
const isNew = id === 'new';
const loading = ref(!isNew);
const notFound = ref(false);
const deleteOpen = ref(false);
const base = ref<KnowledgeBaseResponse | null>(null);
const canManage = computed(() => (isNew ? store.hasPermission(ADMIN_RAG_EDIT) : !!base.value?.can_manage));
const canEdit = computed(() => (isNew ? canManage.value : !!base.value?.can_edit));
const canSearch = computed(() => canManage.value || !!base.value?.usable);
const tabs = computed(() => (isNew ? ['general', 'source'] : ['general', ...(canManage.value ? ['source', 'sharing', 'access'] : []), ...(canEdit.value ? ['models'] : []), ...(canSearch.value ? ['search'] : [])]));
const currentTab = ref(String(route.query.tab ?? 'general'));
const sourceOpened = ref(false);
watch(currentTab, tab => (sourceOpened.value ||= tab === 'source'), {immediate: true});
const pageTitle = computed(() => (isNew ? t('new_title') : form.name || base.value?.name || ''));

const form = reactive({name: '', description: '', view: emptyView() as ExternalView});

function fill(value: KnowledgeBaseResponse) {
	base.value = value;
	form.name = value.name;
	form.description = value.description ?? '';
	if (value.view) form.view = structuredClone(toRaw(value.view));
}

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	canEdit: () => canEdit.value,
	savedKey: 'knowledge.editor.saved',
	errorKey: 'knowledge.editor.save_error',
	validate: () => {
		if (!form.name.trim()) return 'knowledge.editor.name_required';
		if (canManage.value && (!form.view.connection_id || !form.view.collection || !form.view.fields.text || !form.view.embedding.model_id)) return 'knowledge.editor.source_required';
		return null;
	},
	save: async () => {
		const body = {name: form.name.trim(), description: form.description.trim() || null, ...(canManage.value ? {view: form.view} : {})};
		try {
			if (isNew) {
				const created = await createKnowledgeBase(body);
				fill(created);
				settingsForm.markSaved();
				await router.replace(`/settings/knowledge/${created.id}?tab=sharing`);
			} else {
				fill(await updateKnowledgeBase(id, body));
			}
		} catch (error: any) {
			throw new Error(ragErrorMessage(error, ''));
		}
	},
});

async function remove() {
	try {
		await deleteKnowledgeBase(id);
		settingsForm.markSaved();
		deleteOpen.value = false;
		await router.push('/settings/knowledge');
	} catch (error: any) {
		store.toast(t('delete_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

onMounted(async () => {
	if (!isNew) {
		try {
			fill(await getKnowledgeBase(id));
		} catch {
			notFound.value = true;
		} finally {
			loading.value = false;
		}
	}
	settingsForm.markSaved();
});
</script>
