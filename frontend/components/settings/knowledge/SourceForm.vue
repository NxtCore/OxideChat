<template>
	<div class="space-y-6" data-testid="knowledge-source">
		<p class="text-sm text-muted-foreground">{{ t('intro') }}</p>

		<section class="space-y-2">
			<ShadLabel>{{ t('step_connection') }}</ShadLabel>
			<p v-if="connections.length === 0" class="text-sm text-muted-foreground" data-testid="knowledge-source-no-connections">
				{{ t('no_connections') }}
				<NuxtLink to="/settings/knowledge-setup" class="text-primary underline">{{ t('to_setup') }}</NuxtLink>
			</p>
			<ShadSelect v-else :model-value="view.connection_id || undefined" @update:model-value="pickConnection(String($event))">
				<ShadSelectTrigger data-testid="knowledge-source-connection"><ShadSelectValue :placeholder="t('pick')" /></ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem v-for="connection in connections" :key="connection.id" :value="connection.id">{{ connection.name }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
		</section>

		<section v-if="view.connection_id" class="space-y-2">
			<ShadLabel>{{ t('step_collection') }}</ShadLabel>
			<p v-if="storeError" class="text-sm text-destructive" data-testid="knowledge-source-error">{{ store.getTranslation(`rag.store_error.${storeError}`) }}</p>
			<ShadSelect :model-value="view.collection || undefined" @update:model-value="pickCollection(String($event))">
				<ShadSelectTrigger data-testid="knowledge-source-collection"><ShadSelectValue :placeholder="t('pick')" /></ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem v-for="name in collectionNames" :key="name" :value="name">{{ name }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
		</section>

		<template v-if="info">
			<section class="space-y-2">
				<ShadLabel>{{ t('step_sample') }}</ShadLabel>
				<p class="text-xs text-muted-foreground">{{ t('sample_hint') }}</p>
				<p v-if="info.sample.length === 0" class="text-sm text-muted-foreground">{{ t('sample_empty') }}</p>
				<pre v-for="(record, index) in info.sample" :key="index" class="max-h-48 overflow-auto rounded-md border border-border bg-muted/40 p-3 text-xs" data-testid="knowledge-source-sample">{{ JSON.stringify(record, null, 2) }}</pre>
			</section>

			<section class="space-y-3">
				<ShadLabel>{{ t('step_fields') }}</ShadLabel>
				<div class="grid gap-3 sm:grid-cols-2">
					<div class="space-y-1">
						<span class="text-xs text-muted-foreground">{{ t('vector') }}</span>
						<ShadSelect :model-value="view.vector_name ?? DEFAULT_VECTOR" @update:model-value="pickVector(String($event))">
							<ShadSelectTrigger data-testid="knowledge-source-vector"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="vector in info.vectors" :key="vector.name ?? DEFAULT_VECTOR" :value="vector.name ?? DEFAULT_VECTOR">
									{{ vector.name ?? t('default_vector') }}{{ vector.size ? ` · ${t('dimensions', {size: String(vector.size)})}` : '' }}
								</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div v-if="kind === 'pgvector'" class="space-y-1">
						<span class="text-xs text-muted-foreground">{{ t('distance') }}</span>
						<ShadSelect v-model="view.distance">
							<ShadSelectTrigger data-testid="knowledge-source-distance"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="distance in DISTANCES" :key="distance" :value="distance">{{ t(`distance_${distance}`) }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div v-for="field in FIELDS" :key="field" class="space-y-1">
						<span class="text-xs text-muted-foreground">{{ t(`field_${field}`) }}</span>
						<ShadSelect :model-value="view.fields[field] || NONE" @update:model-value="setField(field, String($event))">
							<ShadSelectTrigger :data-testid="`knowledge-source-field-${field}`"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-if="field !== 'text'" :value="NONE">{{ t('field_none') }}</ShadSelectItem>
								<ShadSelectItem v-for="path in info.fields" :key="path" :value="path">{{ path }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
				</div>
			</section>

			<section class="space-y-2">
				<ShadLabel>{{ t('step_filter') }}</ShadLabel>
				<p class="text-xs text-muted-foreground">{{ t('filter_hint') }}</p>
				<div v-for="(condition, index) in filterRows" :key="index" class="flex items-center gap-2">
					<ShadSelect v-model="condition.field">
						<ShadSelectTrigger class="w-48" :data-testid="`knowledge-source-filter-field-${index}`"><ShadSelectValue :placeholder="t('pick')" /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="path in info.fields" :key="path" :value="path">{{ path }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
					<span class="text-sm text-muted-foreground">=</span>
					<ShadInput v-model="condition.value" class="flex-1" :data-testid="`knowledge-source-filter-value-${index}`" />
					<ShadButton variant="ghost" size="icon" :title="t('filter_remove')" @click="filterRows.splice(index, 1)"><X class="h-4 w-4" /></ShadButton>
				</div>
				<div class="flex gap-2">
					<ShadButton variant="outline" size="sm" data-testid="knowledge-source-filter-add" @click="filterRows.push({field: '', value: ''})">{{ t('filter_add') }}</ShadButton>
					<ShadButton variant="outline" size="sm" data-testid="knowledge-source-filter-apply" @click="inspect">{{ t('filter_apply') }}</ShadButton>
				</div>
			</section>

			<section class="space-y-2">
				<ShadLabel>{{ t('step_model') }}</ShadLabel>
				<DefaultModelPicker
					v-model="embeddingModel"
					:endpoint="adminModelsPath"
					:selected-model-endpoint="adminModelsPath"
					value-mode="uuid"
					:placeholder="t('model_none')"
					:params="{type: 'embedding'}"
				/>
				<p class="text-xs text-muted-foreground">{{ vectorSize ? t('model_hint_size', {size: String(vectorSize)}) : t('model_hint') }}</p>
				<ShadInput v-model.number="dimensions" type="number" min="1" max="4000" class="w-32" :placeholder="t('dimensions_placeholder')" data-testid="knowledge-source-dimensions" />
			</section>

			<p class="flex items-start gap-2 rounded-lg border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground" data-testid="knowledge-source-access-note">
				<Info class="mt-px h-3.5 w-3.5 shrink-0" />
				{{ t('access_note') }}
			</p>
		</template>
	</div>
</template>

<script setup lang="ts">
import {Info, X} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import type {CollectionInfo} from '~/types/generated/CollectionInfo';
import type {ConnectionResponse} from '~/types/generated/ConnectionResponse';
import type {Distance} from '~/types/generated/Distance';
import type {ExternalView} from '~/types/generated/ExternalView';
import type {StoreError} from '~/types/generated/StoreError';
import {parseFilterValue} from '~/utils/knowledge';

const NONE = '__none';
const DEFAULT_VECTOR = '__default';
const FIELDS = ['text', 'title', 'url', 'section'] as const;
const DISTANCES: Distance[] = ['cosine', 'dot', 'euclid', 'manhattan'];

const view = defineModel<ExternalView>({required: true});
const store = useMainStore();
const t = (key: string, params?: Record<string, string>) => store.getTranslation(`knowledge.source.${key}`, params);

const connections = ref<ConnectionResponse[]>([]);
const collections = ref<string[]>([]);
const info = ref<CollectionInfo | null>(null);
const storeError = ref<StoreError | null>(null);
const filterRows = ref(view.value.filter.map(condition => ({field: condition.field, value: String(condition.value)})));

const kind = computed(() => connections.value.find(connection => connection.id === view.value.connection_id)?.kind);
const collectionNames = computed(() => [...new Set([...collections.value, ...(view.value.collection ? [view.value.collection] : [])])]);
const vectorSize = computed(() => info.value?.vectors.find(vector => vector.name === view.value.vector_name)?.size ?? null);
const embeddingModel = computed({
	get: () => view.value.embedding.model_id || null,
	set: (value: string | null) => (view.value.embedding.model_id = value ?? ''),
});
const dimensions = computed({
	get: () => view.value.embedding.dimensions ?? undefined,
	set: (value: number | string | undefined) => (view.value.embedding.dimensions = typeof value === 'number' && value > 0 ? value : null),
});

watch(
	filterRows,
	rows => (view.value.filter = rows.filter(row => row.field && row.value.trim()).map(row => ({field: row.field, value: parseFilterValue(row.value)}))),
	{deep: true},
);

function setField(field: (typeof FIELDS)[number], value: string) {
	if (field === 'text') view.value.fields.text = value === NONE ? '' : value;
	else view.value.fields[field] = value === NONE ? null : value;
}

function pickVector(value: string) {
	view.value.vector_name = value === DEFAULT_VECTOR ? null : value;
	const vector = info.value?.vectors.find(candidate => candidate.name === view.value.vector_name);
	if (vector?.distance) view.value.distance = vector.distance;
}

async function loadCollections() {
	storeError.value = null;
	collections.value = [];
	try {
		const result = await testRagConnection(view.value.connection_id);
		collections.value = result.collections;
		storeError.value = result.error;
	} catch (error: any) {
		store.toast(t('load_error'), {type: 'error', description: errorMessage(error, '')});
	}
}

async function inspect() {
	if (!view.value.connection_id || !view.value.collection) return;
	try {
		const result = await inspectCollection(view.value.connection_id, {collection: view.value.collection, filter: view.value.filter});
		storeError.value = result.error;
		info.value = result.collection;
	} catch (error: any) {
		store.toast(t('load_error'), {type: 'error', description: ragErrorMessage(error, '')});
	}
}

async function pickConnection(id: string) {
	Object.assign(view.value, {connection_id: id, collection: '', vector_name: null, filter: []});
	filterRows.value = [];
	info.value = null;
	await loadCollections();
}

async function pickCollection(name: string) {
	view.value.collection = name;
	await inspect();
	const first = info.value?.vectors[0];
	if (first && !info.value?.vectors.some(vector => vector.name === view.value.vector_name)) pickVector(first.name ?? DEFAULT_VECTOR);
}


onMounted(async () => {
	try {
		connections.value = (await ragSetup()).connections.filter(connection => connection.mode === 'read_only');
	} catch (error: any) {
		store.toast(t('load_error'), {type: 'error', description: errorMessage(error, '')});
	}
	if (view.value.connection_id) {
		await loadCollections();
		await inspect();
	}
});
</script>
