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
});

const managedConnections = computed(() => setup.value?.connections.filter(connection => connection.mode === 'managed') ?? []);
const defaultConnection = computed({
	get: () => form.defaults.connection_id ?? NONE,
	set: (value: string) => (form.defaults.connection_id = value === NONE ? null : value),
});

function fill(value: RagSetupResponse) {
	form.reranker = {...value.reranker};
	form.defaults = {...value.defaults};
}

function apply(value: RagSetupResponse) {
	const dirty = settingsForm.isDirty;
	setup.value = value;
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
	save: async () => {
		try {
			setup.value = await saveRagSetup(form);
		} catch (error: any) {
			throw new Error(ragErrorMessage(error, ''));
		}
	},
});

onMounted(async () => {
	const value = await ragSetup();
	setup.value = value;
	fill(value);
	settingsForm.markSaved();
});
</script>
