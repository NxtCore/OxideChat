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

			<SettingsKnowledgeDocuments v-if="!isNew && isManaged && currentTab === 'documents'" :id="id" :can-edit="canEdit" />

			<div v-show="currentTab === 'general'" class="space-y-4">
				<div v-if="isNew && canCreateManaged && canCreateExternal" class="space-y-2">
					<ShadLabel>{{ t('kind_label') }}</ShadLabel>
					<ShadSelect v-model="createKind">
						<ShadSelectTrigger data-testid="knowledge-kind"><ShadSelectValue /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="managed">{{ t('kind_managed') }}</ShadSelectItem>
							<ShadSelectItem value="external">{{ t('kind_external') }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>

				<div class="space-y-2">
					<ShadLabel for="knowledge-name">{{ t('name') }}</ShadLabel>
					<ShadInput id="knowledge-name" v-model="form.name" maxlength="100" :readonly="!canEdit" data-testid="knowledge-name" />
				</div>

				<div class="space-y-2">
					<ShadLabel for="knowledge-description">{{ t('description_label') }}</ShadLabel>
					<SettingsMarkdownField id="knowledge-description" v-model="form.description" :maxlength="2000" :readonly="!canEdit" />
					<p class="text-xs text-muted-foreground">{{ t('description_hint') }}</p>
				</div>

				<template v-if="isNew && isManaged && scopeOptions.length > 1">
					<div class="space-y-2">
						<ShadLabel>{{ t('scope_label') }}</ShadLabel>
						<ShadSelect v-model="form.scope">
							<ShadSelectTrigger data-testid="knowledge-scope"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="option in scopeOptions" :key="option" :value="option">
									{{ store.getTranslation(`knowledge.list.scope_${option}`) }}
								</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
				</template>

				<div v-if="isNew && isManaged && form.scope === 'team' && (scopes?.teams.length ?? 0) > 0" class="space-y-2">
					<ShadLabel>{{ t('team_label') }}</ShadLabel>
					<ShadSelect v-model="form.team_id">
						<ShadSelectTrigger data-testid="knowledge-team"><ShadSelectValue /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="team in scopes?.teams ?? []" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>

				<div v-if="isManaged" class="space-y-2">
					<ShadLabel>{{ t('language_label') }}</ShadLabel>
					<ShadSelect v-model="form.search_language" :disabled="!canEdit">
						<ShadSelectTrigger class="w-48" data-testid="knowledge-language"><ShadSelectValue /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="lang in SEARCH_LANGUAGES" :key="lang" :value="lang">{{ lang }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
					<p class="text-xs text-muted-foreground">{{ t('language_hint') }}</p>
				</div>
			</div>

			<SettingsKnowledgeSourceForm v-if="!isManaged && canView && sourceOpened" v-show="currentTab === 'source'" v-model="form.view" :readonly="!canAdminManage" />

			<SettingsKnowledgeSharing v-if="!isNew && canShareTab && currentTab === 'sharing'" :id="id" :per-record="!!base?.view?.access" :managed="isManaged" />

			<SettingsKnowledgeAccessLabels v-if="!isNew && !isManaged && canView && currentTab === 'access'" :id="id" :can-manage="canAdminManage" />

			<SettingsKnowledgeModelAccess v-if="!isNew && canSeeModels && currentTab === 'models'" :data="modelAccess" :form="form" @confirm="confirmModels" />

			<SettingsKnowledgeTestSearch v-if="!isNew && canSearch && currentTab === 'search'" :id="id" :can-search-as="canAdminManage" />
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
import {ADMIN_RAG_EDIT, ADMIN_RAG_VIEW} from '~/types/generated/permissions';
import type {ExternalView} from '~/types/generated/ExternalView';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import type {KnowledgeScope} from '~/types/generated/KnowledgeScope';
import type {KnowledgeScopesResponse} from '~/types/generated/KnowledgeScopesResponse';
import type {ModelAccessResponse} from '~/types/generated/ModelAccessResponse';
import type {ModelTargets} from '~/types/generated/ModelTargets';
import {SEARCH_LANGUAGES, emptyView} from '~/utils/knowledge';

const store = useMainStore();
const route = useRoute();
const router = useRouter();
const t = (key: string) => store.getTranslation(`knowledge.editor.${key}`);

const id = String(route.params.id);
const isNew = id === 'new';
const loading = ref(true);
const notFound = ref(false);
const deleteOpen = ref(false);
const base = ref<KnowledgeBaseResponse | null>(null);
const scopes = ref<KnowledgeScopesResponse | null>(null);
const createKind = ref<'managed' | 'external'>('external');

const canAdminManage = computed(() => store.hasPermission(ADMIN_RAG_EDIT));
const canCreateManaged = computed(() =>
	Boolean(scopes.value?.managed_available && (scopes.value.personal || scopes.value.teams.length > 0 || scopes.value.installation)),
);
const canCreateExternal = computed(() => Boolean(scopes.value?.external));
const isManaged = computed(() => (isNew ? createKind.value === 'managed' : base.value?.mode === 'managed'));

const scopeOptions = computed<KnowledgeScope[]>(() => {
	if (!scopes.value) return ['personal'];
	const list: KnowledgeScope[] = [];
	if (scopes.value.personal) list.push('personal');
	if (scopes.value.teams.length > 0) list.push('team');
	if (scopes.value.installation) list.push('installation');
	return list;
});

const canManage = computed(() => (isNew ? canCreateManaged.value || canCreateExternal.value : !!base.value?.can_manage));
const canEdit = computed(() => (isNew ? canManage.value : !!base.value?.can_edit));
const canView = computed(() => canAdminManage.value || store.hasPermission(ADMIN_RAG_VIEW));
const canShareTab = computed(() => canView.value || !!base.value?.can_manage);
const canSeeModels = computed(() => canEdit.value || canView.value);
const canSearch = computed(() => canManage.value || !!base.value?.usable);

const tabs = computed(() => {
	if (isNew) return isManaged.value ? ['general'] : ['general', 'source'];
	if (isManaged.value) {
		return ['documents', 'general', ...(canShareTab.value ? ['sharing'] : []), ...(canSeeModels.value ? ['models'] : []), ...(canSearch.value ? ['search'] : [])];
	}
	return ['general', ...(canView.value ? ['source', 'sharing', 'access'] : []), ...(canSeeModels.value ? ['models'] : []), ...(canSearch.value ? ['search'] : [])];
});

const currentTab = ref(String(route.query.tab ?? 'general'));
const sourceOpened = ref(false);
watch(currentTab, tab => (sourceOpened.value ||= tab === 'source'), {immediate: true});
const pageTitle = computed(() => (isNew ? t('new_title') : form.name || base.value?.name || ''));

const form = reactive({
	name: '',
	description: '',
	scope: 'personal' as KnowledgeScope,
	team_id: '' as string,
	search_language: 'simple',
	view: emptyView() as ExternalView,
	models: {providers: [], upstream_providers: [], models: []} as ModelTargets,
});
const modelAccess = ref<ModelAccessResponse | null>(null);

watch(
	() => form.scope,
	scope => {
		if (scope === 'team' && !form.team_id && scopes.value?.teams[0]) form.team_id = scopes.value.teams[0].id;
	},
);

function fillModels(value: ModelAccessResponse) {
	modelAccess.value = value;
	form.models = structuredClone(toRaw(value.targets));
}

const modelsChanged = () => !!modelAccess.value?.can_edit && JSON.stringify(form.models) !== JSON.stringify(modelAccess.value.targets);

function fill(value: KnowledgeBaseResponse) {
	base.value = value;
	form.name = value.name;
	form.description = value.description ?? '';
	form.scope = value.scope;
	form.team_id = value.team_id ?? '';
	form.search_language = value.search_language || 'simple';
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
		if (!isManaged.value && canAdminManage.value && (!form.view.connection_id || !form.view.collection || !form.view.fields.text || !form.view.embedding.model_id)) {
			return 'knowledge.editor.source_required';
		}
		return null;
	},
	save: async () => {
		const models = !isNew && modelsChanged() ? toRaw(form.models) : null;
		const body = isManaged.value
			? {
					name: form.name.trim(),
					description: form.description.trim() || null,
					search_language: form.search_language,
					...(isNew ? {scope: form.scope, ...(form.scope === 'team' ? {team_id: form.team_id || null} : {})} : {}),
					...(models ? {model_access: models} : {}),
				}
			: {
					name: form.name.trim(),
					description: form.description.trim() || null,
					...(canAdminManage.value ? {view: form.view} : {}),
					...(models ? {model_access: models} : {}),
				};
		try {
			if (isNew) {
				const created = await createKnowledgeBase(body);
				fill(created);
				settingsForm.markSaved();
				await router.replace(`/settings/knowledge/${created.id}?tab=${created.mode === 'managed' ? 'documents' : 'sharing'}`);
			} else {
				fill(await updateKnowledgeBase(id, body));
				if (models) fillModels(await knowledgeModelAccess(id));
			}
		} catch (error: any) {
			throw new Error(ragErrorMessage(error, ''));
		}
	},
});

async function confirmModels() {
	try {
		fillModels(await saveKnowledgeModelAccess(id, toRaw(form.models)));
		store.toast(store.getTranslation('knowledge.editor.saved'), {type: 'success'});
	} catch (error: any) {
		store.toast(store.getTranslation('knowledge.editor.save_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

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
	try {
		if (isNew) {
			scopes.value = await knowledgeScopes().catch(() => null);
			if (canCreateManaged.value && (!canCreateExternal.value || route.query.kind === 'managed' || scopes.value?.managed_available)) {
				createKind.value = route.query.kind === 'external' && canCreateExternal.value ? 'external' : 'managed';
			} else {
				createKind.value = 'external';
			}
			if (scopeOptions.value[0]) form.scope = scopeOptions.value[0];
			if (form.scope === 'team' && scopes.value?.teams[0]) form.team_id = scopes.value.teams[0].id;
		} else {
			fill(await getKnowledgeBase(id));
			if (!route.query.tab && base.value?.mode === 'managed') currentTab.value = 'documents';
			if (canSeeModels.value) fillModels(await knowledgeModelAccess(id));
		}
	} catch {
		notFound.value = true;
	} finally {
		loading.value = false;
		settingsForm.markSaved();
	}
});
</script>
