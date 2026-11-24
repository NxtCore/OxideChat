<template>
	<div class="mt-6 rounded-lg border border-border bg-card p-6" data-testid="knowledge-connections">
		<div class="mb-4 flex items-start justify-between gap-4">
			<div>
				<h3 class="text-sm font-semibold text-foreground">{{ t('title') }}</h3>
				<p class="mt-0.5 text-xs text-muted-foreground">{{ t('description') }}</p>
			</div>
			<ShadButton v-if="canEdit" size="sm" data-testid="knowledge-connection-add" @click="openCreate">{{ t('add') }}</ShadButton>
		</div>

		<p v-if="setup.connections.length === 0" class="text-sm text-muted-foreground" data-testid="knowledge-connections-empty">{{ t('empty') }}</p>

		<SettingsRow v-for="item in setup.connections" :key="item.id" :label="item.name" :hint="summary(item)" :data-testid="`knowledge-connection-${item.name}`">
			<div class="flex flex-col items-end gap-1">
				<div class="flex gap-2">
					<ShadButton variant="outline" size="sm" :disabled="testing.has(item.id)" :data-testid="`knowledge-connection-test-${item.name}`" @click="test(item)">{{ t('test') }}</ShadButton>
					<ShadButton variant="outline" size="sm" :disabled="!canEdit" :data-testid="`knowledge-connection-configure-${item.name}`" @click="openEdit(item)">{{ t('configure') }}</ShadButton>
				</div>
				<p v-if="results[item.id]" class="max-w-xs text-right text-xs" :class="results[item.id]!.ok ? 'text-muted-foreground' : 'text-destructive'" :data-testid="`knowledge-connection-result-${item.name}`">
					{{ resultText(results[item.id]!) }}
				</p>
			</div>
		</SettingsRow>

		<ShadDialog v-model:open="dialogOpen">
			<ShadDialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-[560px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ editing ? editing.name : t('add') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('description') }}</ShadDialogDescription>
				</ShadDialogHeader>

				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<ShadLabel for="knowledge-connection-name">{{ t('name') }}</ShadLabel>
						<ShadInput id="knowledge-connection-name" v-model="form.name" data-testid="knowledge-connection-name" autocomplete="off" />
					</div>

					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<ShadLabel>{{ t('kind') }}</ShadLabel>
							<ShadSelect v-model="form.kind" :disabled="Boolean(editing)">
								<ShadSelectTrigger data-testid="knowledge-connection-kind"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="pgvector">pgvector</ShadSelectItem>
									<ShadSelectItem value="qdrant">Qdrant</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-2">
							<ShadLabel>{{ t('mode') }}</ShadLabel>
							<ShadSelect v-model="form.mode" :disabled="Boolean(editing) || form.kind === 'qdrant'">
								<ShadSelectTrigger data-testid="knowledge-connection-mode"><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="managed">{{ t('mode_managed') }}</ShadSelectItem>
									<ShadSelectItem value="read_only">{{ t('mode_read_only') }}</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
					</div>
					<p v-if="!editing" class="-mt-2 text-xs text-muted-foreground">{{ t('mode_hint') }}</p>

					<div class="space-y-2">
						<ShadLabel for="knowledge-connection-url">{{ t('url') }}</ShadLabel>
						<ShadInput id="knowledge-connection-url" v-model="form.url" data-testid="knowledge-connection-url" autocomplete="off" :placeholder="form.kind === 'pgvector' && form.mode === 'managed' ? t('main_database') : ''" />
						<p class="text-xs text-muted-foreground">{{ editing?.url_from_env ? t('url_from_env') : t(`url_hint_${form.kind}`) }}</p>
					</div>

					<div class="space-y-2">
						<ShadLabel for="knowledge-connection-secret">{{ t(`secret_${form.kind}`) }}</ShadLabel>
						<ShadInput
							id="knowledge-connection-secret"
							v-model="form.secret"
							type="password"
							data-testid="knowledge-connection-secret"
							autocomplete="new-password"
							:placeholder="editing?.has_secret ? '••••••••' : ''"
							:disabled="form.clearSecret"
						/>
						<p class="text-xs text-muted-foreground">{{ t('secret_hint') }}</p>
						<label v-if="editing?.has_secret" class="flex items-center gap-2 text-xs">
							<ShadCheckbox v-model="form.clearSecret" data-testid="knowledge-connection-clear-secret" />
							{{ t('clear_secret') }}
						</label>
					</div>

					<template v-if="form.mode === 'managed'">
						<div class="space-y-2">
							<ShadLabel>{{ t('embedding_model') }}</ShadLabel>
							<DefaultModelPicker
								v-model="form.embeddingModelId"
								:endpoint="adminModelsPath"
								:selected-model-endpoint="adminModelsPath"
								value-mode="uuid"
								:placeholder="t('embedding_none')"
								:params="{type: 'embedding'}"
							/>
							<p class="text-xs text-muted-foreground">{{ t('embedding_model_hint') }}</p>
						</div>
						<div v-if="form.embeddingModelId" class="space-y-2">
							<ShadLabel for="knowledge-connection-dimensions">{{ t('dimensions') }}</ShadLabel>
							<ShadInput id="knowledge-connection-dimensions" v-model.number="form.dimensions" type="number" min="1" max="4000" class="w-32" data-testid="knowledge-connection-dimensions" />
							<p class="text-xs text-muted-foreground">{{ t('dimensions_hint') }}</p>
						</div>
					</template>
				</div>

				<ShadDialogFooter class="gap-2 sm:gap-0">
					<ShadButton v-if="editing" variant="destructive" class="mr-auto" :disabled="saving" data-testid="knowledge-connection-remove" @click="remove">
						{{ confirmDelete ? t('delete_confirm') : store.getTranslation('common.delete') }}
					</ShadButton>
					<div class="flex flex-row gap-2">
						<ShadButton variant="outline" @click="dialogOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
						<ShadButton :disabled="form.name.trim() === '' || saving" data-testid="knowledge-connection-save" @click="save">{{ store.getTranslation('common.save') }}</ShadButton>
					</div>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {reactive, ref, watch} from 'vue';
import {useMainStore} from '@/stores';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import type {RagSetupResponse} from '~/types/generated/RagSetupResponse';
import type {ConnectionResponse} from '~/types/generated/ConnectionResponse';
import type {ConnectionTestResponse} from '~/types/generated/ConnectionTestResponse';
import type {ConnectionKind} from '~/types/generated/ConnectionKind';
import type {ConnectionMode} from '~/types/generated/ConnectionMode';

defineProps<{setup: RagSetupResponse; canEdit: boolean}>();
const emit = defineEmits<{changed: [value: RagSetupResponse]}>();

const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`settings.knowledge_setup.connections.${key}`, params);

const dialogOpen = ref(false);
const saving = ref(false);
const editing = ref<ConnectionResponse | null>(null);
const testing = reactive(new Set<string>());
const confirmDelete = ref(false);
const results = reactive<Record<string, ConnectionTestResponse>>({});

const form = reactive({
	name: '',
	kind: 'pgvector' as ConnectionKind,
	mode: 'managed' as ConnectionMode,
	url: '',
	secret: '',
	clearSecret: false,
	embeddingModelId: null as string | null,
	dimensions: null as number | null,
});

watch(
	() => form.kind,
	kind => {
		if (kind === 'qdrant') form.mode = 'read_only';
	},
);

function summary(item: ConnectionResponse): string {
	const parts = [item.kind === 'qdrant' ? 'Qdrant' : 'pgvector', t(`mode_${item.mode}`), item.url_from_env ? 'RAG_PGVECTOR_URL' : (item.url ?? t('main_database'))];
	if (item.embedding) parts.push(t('vector_size', {size: String(item.embedding.vector_size)}));
	return parts.join(' · ');
}

function resultText(result: ConnectionTestResponse): string {
	if (!result.ok) return `${t('test_failed')}: ${store.getTranslation(`rag.store_error.${result.error}`)}`;
	const collections = result.collections.length > 0 ? t('collections', {names: result.collections.join(', ')}) : '';
	return [`${t('test_ok')} (${result.version})`, collections].filter(Boolean).join(' · ');
}

function openCreate() {
	editing.value = null;
	confirmDelete.value = false;
	Object.assign(form, {name: '', kind: 'pgvector', mode: 'managed', url: '', secret: '', clearSecret: false, embeddingModelId: null, dimensions: null});
	dialogOpen.value = true;
}

function openEdit(item: ConnectionResponse) {
	editing.value = item;
	confirmDelete.value = false;
	Object.assign(form, {
		name: item.name,
		kind: item.kind,
		mode: item.mode,
		url: item.url ?? '',
		secret: '',
		clearSecret: false,
		embeddingModelId: item.embedding?.model_id ?? null,
		dimensions: item.embedding?.dimensions ?? null,
	});
	dialogOpen.value = true;
}

function requestBody() {
	const secret = form.clearSecret ? '' : form.secret || null;
	return {
		name: form.name.trim(),
		kind: form.kind,
		mode: form.mode,
		url: form.url.trim() || null,
		secret,
		embedding: form.mode === 'managed' && form.embeddingModelId ? {model_id: form.embeddingModelId, dimensions: form.dimensions && form.dimensions > 0 ? Math.round(form.dimensions) : null} : null,
	};
}

async function run(action: () => Promise<RagSetupResponse>, successKey: string, errorKey: string, id?: string) {
	saving.value = true;
	try {
		emit('changed', await action());
		if (id) delete results[id];
		dialogOpen.value = false;
		store.toast(t(successKey), {type: 'success'});
	} catch (error: any) {
		store.toast(t(errorKey), {type: 'error', description: ragErrorMessage(error, '')});
	} finally {
		saving.value = false;
	}
}

function save() {
	const body = requestBody();
	const target = editing.value;
	run(() => (target ? updateRagConnection(target.id, body) : createRagConnection(body)), 'saved', 'save_error', target?.id);
}

function remove() {
	const target = editing.value;
	if (!target) return;
	if (!confirmDelete.value) {
		confirmDelete.value = true;
		return;
	}
	run(() => deleteRagConnection(target.id), 'deleted', 'delete_error', target.id);
}

async function test(item: ConnectionResponse) {
	testing.add(item.id);
	try {
		results[item.id] = await testRagConnection(item.id);
	} catch (error: any) {
		store.toast(t('test_failed'), {type: 'error', description: errorMessage(error, '')});
	} finally {
		testing.delete(item.id);
	}
}
</script>
