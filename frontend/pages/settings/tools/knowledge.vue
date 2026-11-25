<template>
	<SettingsPage :title="store.getTranslation('settings.tabs.knowledge_setup')" :description="store.getTranslation('settings.knowledge_setup.description')" layout="form" :form="settingsForm">
		<template #leading>
			<ShadButton variant="ghost" size="icon" class="shrink-0" :aria-label="store.getTranslation('settings.tabs.tools')" data-testid="knowledge-setup-back" @click="navigateTo('/settings/tools')">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>
		</template>
		<template v-if="setup">
			<div class="rounded-lg border border-border bg-card p-6">
				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.enabled')" :hint="store.getTranslation('settings.knowledge_setup.enabled_hint')">
					<NuxtLink to="/settings/tools" class="text-sm text-primary underline" data-testid="knowledge-status">
						{{ store.getTranslation(store.base?.rag?.enabled ? 'settings.knowledge_setup.status_on' : 'settings.knowledge_setup.status_off') }}
					</NuxtLink>
				</SettingsRow>

				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.default_connection')" :hint="store.getTranslation('settings.knowledge_setup.default_connection_hint')" stacked>
					<ShadSelect v-model="defaultConnection" :disabled="!canEdit || settingsForm.saving">
						<ShadSelectTrigger data-testid="knowledge-default-connection">
							<ShadSelectValue />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem :value="NONE">{{ store.getTranslation('settings.knowledge_setup.none') }}</ShadSelectItem>
							<ShadSelectItem v-for="connection in managedConnections" :key="connection.id" :value="connection.id">{{ connection.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</SettingsRow>

				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.reranker')" :hint="store.getTranslation('settings.knowledge_setup.reranker_hint')" stacked>
					<DefaultModelPicker
						v-model="form.reranker.model_id"
						:disabled="!canEdit || settingsForm.saving"
						:endpoint="adminModelsPath"
						:selected-model-endpoint="adminModelsPath"
						value-mode="uuid"
						:placeholder="store.getTranslation('settings.knowledge_setup.reranker_none')"
						:params="{type: 'reranking'}"
					/>
				</SettingsRow>

				<SettingsRow v-if="form.reranker.model_id" :label="store.getTranslation('settings.knowledge_setup.top_n')">
					<ShadInput v-model.number="form.reranker.top_n" type="number" min="1" max="50" class="w-24" :disabled="!canEdit || settingsForm.saving" data-testid="knowledge-top-n" />
				</SettingsRow>
			</div>

			<div class="mt-6 rounded-lg border border-border bg-card p-6">
				<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.knowledge_setup.chunking_title') }}</h3>
				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.chunk_size')" :hint="store.getTranslation('settings.knowledge_setup.chunk_size_hint')">
					<ShadInput v-model.number="form.chunking.chunk_size" type="number" min="100" max="2000" class="w-28" :disabled="!canEdit || settingsForm.saving" data-testid="knowledge-chunk-size" />
				</SettingsRow>
				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.chunk_overlap')" :hint="store.getTranslation('settings.knowledge_setup.chunk_overlap_hint')">
					<ShadInput v-model.number="form.chunking.overlap_percent" type="number" min="0" max="50" class="w-28" :disabled="!canEdit || settingsForm.saving" data-testid="knowledge-chunk-overlap" />
				</SettingsRow>
				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.max_uploads_per_hour')">
					<ShadInput v-model.number="form.quotas.max_uploads_per_hour" type="number" min="1" max="10000" class="w-28" :disabled="!canEdit || settingsForm.saving" data-testid="knowledge-max-uploads" />
				</SettingsRow>
				<SettingsRow :label="store.getTranslation('settings.knowledge_setup.storage_quota_mb')" :hint="store.getTranslation('settings.knowledge_setup.storage_quota_hint')">
					<ShadInput v-model="storageQuota" type="number" min="1" class="w-28" :disabled="!canEdit || settingsForm.saving" data-testid="knowledge-storage-quota" />
				</SettingsRow>
			</div>

			<div v-if="setup.connections.length === 0" class="mt-6 rounded-lg border border-border bg-card p-6" data-testid="knowledge-setup-steps">
				<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.knowledge_setup.steps.title') }}</h3>
				<ol class="mt-3 list-decimal space-y-1 pl-5 text-sm text-muted-foreground">
					<li v-for="step in ['connection', 'test', 'collection']" :key="step">{{ store.getTranslation(`settings.knowledge_setup.steps.${step}`) }}</li>
				</ol>
			</div>

			<SettingsKnowledgeConnectionsCard :setup="setup" :can-edit="canEdit" @changed="apply" />
		</template>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ADMIN_RAG_EDIT} from '~/types/generated/permissions';
import {ArrowLeft} from 'lucide-vue-next';
import {computed, onMounted, reactive, ref} from 'vue';
import {useMainStore} from '@/stores';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import type {RagSetupResponse} from '~/types/generated/RagSetupResponse';

const NONE = 'none';

const store = useMainStore();
const canEdit = computed(() => store.hasPermission(ADMIN_RAG_EDIT));
const setup = ref<RagSetupResponse | null>(null);

const form = reactive({
	reranker: {model_id: null as string | null, top_n: 6},
	defaults: {connection_id: null as string | null},
	chunking: {chunk_size: 400, overlap_percent: 15},
	quotas: {max_uploads_per_hour: 60, storage_quota_mb: null as number | null},
});

const storageQuota = computed({
	get: () => (form.quotas.storage_quota_mb === null ? '' : String(form.quotas.storage_quota_mb)),
	set: (value: string) => {
		const trimmed = String(value).trim();
		form.quotas.storage_quota_mb = trimmed === '' ? null : Number(trimmed);
	},
});

const managedConnections = computed(() => setup.value?.connections.filter(connection => connection.mode === 'managed') ?? []);
const defaultConnection = computed({
	get: () => form.defaults.connection_id ?? NONE,
	set: (value: string) => (form.defaults.connection_id = value === NONE ? null : value),
});

function fill(value: RagSetupResponse) {
	form.reranker = {...value.reranker};
	form.defaults = {...value.defaults};
	form.chunking = {...value.chunking};
	form.quotas = {...value.quotas};
}

function apply(value: RagSetupResponse) {
	const dirty = settingsForm.isDirty;
	setup.value = value;
	if (form.defaults.connection_id && !value.connections.some(connection => connection.id === form.defaults.connection_id)) form.defaults.connection_id = null;
	if (!dirty) {
		fill(value);
		settingsForm.markSaved();
	}
}

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	canEdit: () => canEdit.value,
	savedKey: 'settings.knowledge_setup.save_success',
	errorKey: 'settings.knowledge_setup.save_error',
	validate: () => {
		if (!Number.isInteger(form.reranker.top_n) || form.reranker.top_n < 1 || form.reranker.top_n > 50) return 'rag.error.invalid_top_n';
		if (!Number.isInteger(form.chunking.chunk_size) || form.chunking.chunk_size < 100 || form.chunking.chunk_size > 2000) return 'rag.error.invalid_chunking';
		if (!Number.isInteger(form.chunking.overlap_percent) || form.chunking.overlap_percent < 0 || form.chunking.overlap_percent > 50) return 'rag.error.invalid_chunking';
		if (!Number.isInteger(form.quotas.max_uploads_per_hour) || form.quotas.max_uploads_per_hour < 1 || form.quotas.max_uploads_per_hour > 10000) return 'rag.error.invalid_quotas';
		if (form.quotas.storage_quota_mb !== null && (!Number.isInteger(form.quotas.storage_quota_mb) || form.quotas.storage_quota_mb < 1)) return 'rag.error.invalid_quotas';
		return null;
	},
	save: async () => {
		try {
			setup.value = await saveRagSetup(form);
		} catch (error: any) {
			throw new Error(ragErrorMessage(error, ''));
		}
	},
});

onMounted(async () => {
	try {
		const value = await ragSetup();
		setup.value = value;
		fill(value);
		settingsForm.markSaved();
	} catch (error: any) {
		store.toast(store.getTranslation('settings.knowledge_setup.load_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
});
</script>
