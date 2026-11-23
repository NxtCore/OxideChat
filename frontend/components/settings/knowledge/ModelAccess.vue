<template>
	<div class="space-y-5" data-testid="knowledge-models">
		<div class="space-y-1">
			<h3 class="text-base font-semibold text-foreground">{{ t('title') }}</h3>
			<p class="max-w-prose text-sm text-muted-foreground">{{ t('description') }}</p>
		</div>

		<div v-if="!data" class="flex items-center justify-center py-8 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>

		<fieldset v-else :disabled="!data.can_edit || saving" class="space-y-5">
			<p v-if="!data.can_edit" class="text-sm text-muted-foreground" data-testid="knowledge-models-admins-only">{{ t('admins_only') }}</p>
			<p v-if="data.removed_targets" class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive" data-testid="knowledge-models-removed">
				{{ t('removed_targets') }}
			</p>
			<p v-else-if="unrestricted" class="rounded-md border border-dashed border-border px-3 py-2 text-sm text-muted-foreground" data-testid="knowledge-models-unrestricted">
				{{ t('unrestricted') }}
			</p>

			<section class="space-y-2">
				<h4 class="text-sm font-medium text-foreground">{{ t('providers') }}</h4>
				<p class="text-xs text-muted-foreground">{{ t('providers_hint') }}</p>
				<div class="max-h-56 overflow-y-auto rounded-md border border-border p-1">
					<label
						v-for="provider in data.providers"
						:key="provider.id"
						class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
						:data-testid="`knowledge-models-provider-${provider.name}`"
					>
						<ShadCheckbox :model-value="targets.providers.includes(provider.id)" @update:model-value="toggle('providers', provider.id, $event === true)" />
						<span class="truncate">{{ provider.name }}</span>
					</label>
				</div>
			</section>

			<section v-if="routesUpstream" class="space-y-2">
				<h4 class="text-sm font-medium text-foreground">{{ t('upstream') }}</h4>
				<p class="text-xs text-muted-foreground">{{ t('upstream_hint') }}</p>
				<div class="rounded-md border border-border">
					<div class="border-b border-border p-2">
						<ShadInput v-model="slug" class="h-8" :placeholder="t('upstream_placeholder')" data-testid="knowledge-models-upstream-input" @keydown.enter.prevent="addSlug" />
					</div>
					<div class="max-h-56 overflow-y-auto p-1">
						<label v-for="option in upstreamOptions" :key="option" class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent">
							<ShadCheckbox :model-value="targets.upstream_providers.includes(option)" @update:model-value="toggle('upstream_providers', option, $event === true)" />
							<span class="truncate">{{ option }}</span>
						</label>
					</div>
				</div>
			</section>

			<section class="space-y-2">
				<h4 class="text-sm font-medium text-foreground">{{ t('models') }}</h4>
				<p class="text-xs text-muted-foreground">{{ t('models_hint') }}</p>
				<div class="rounded-md border border-border">
					<div class="border-b border-border p-2">
						<ShadInput v-model="search" class="h-8" :placeholder="t('search')" data-testid="knowledge-models-search" />
					</div>
					<div class="max-h-72 overflow-y-auto p-1">
						<label
							v-for="model in visibleModels"
							:key="model.id"
							class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
							:data-testid="`knowledge-models-model-${model.name}`"
						>
							<ShadCheckbox :model-value="targets.models.includes(model.id)" @update:model-value="toggle('models', model.id, $event === true)" />
							<span class="min-w-0 flex-1 truncate">{{ model.name }}</span>
							<span class="shrink-0 text-xs text-muted-foreground">{{ providerName(model.provider_id) }}</span>
						</label>
					</div>
				</div>
			</section>

			<ShadButton v-if="data.can_edit" type="button" data-testid="knowledge-models-save" :disabled="saving" @click="save">{{ t('save') }}</ShadButton>
		</fieldset>
	</div>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {ModelAccessResponse} from '~/types/generated/ModelAccessResponse';
import type {ModelTargets} from '~/types/generated/ModelTargets';

const props = defineProps<{id: string}>();
const store = useMainStore();
const t = (key: string) => store.getTranslation(`knowledge.models.${key}`);
const data = ref<ModelAccessResponse | null>(null);
const targets = reactive<ModelTargets>({providers: [], upstream_providers: [], models: []});
const search = ref('');
const slug = ref('');
const saving = ref(false);

const unrestricted = computed(() => !targets.providers.length && !targets.upstream_providers.length && !targets.models.length);
const routesUpstream = computed(() => data.value?.providers.some(provider => provider.routes_upstream) ?? false);
const upstreamOptions = computed(() => [...new Set([...(data.value?.upstream_providers ?? []), ...targets.upstream_providers])].sort());
const visibleModels = computed(() => {
	const term = search.value.trim().toLowerCase();
	const models = data.value?.models ?? [];
	return term ? models.filter(model => model.name.toLowerCase().includes(term)) : models;
});

function providerName(id: string) {
	return data.value?.providers.find(provider => provider.id === id)?.name ?? '';
}

function fill(value: ModelAccessResponse) {
	data.value = value;
	Object.assign(targets, structuredClone(toRaw(value.targets)));
}

function toggle(list: keyof ModelTargets, value: string, selected: boolean) {
	const rest = targets[list].filter(item => item !== value);
	targets[list] = selected ? [...rest, value] : rest;
}

function addSlug() {
	const value = slug.value.trim().toLowerCase();
	if (!/^[a-z0-9\-_./]{1,64}$/.test(value)) return;
	toggle('upstream_providers', value, true);
	slug.value = '';
}

async function save() {
	saving.value = true;
	try {
		fill(await saveKnowledgeModelAccess(props.id, toRaw(targets)));
		store.toast(t('saved'), {type: 'success'});
	} catch (error: any) {
		store.toast(t('save_error'), {type: 'error', description: ragErrorMessage(error, '')});
	} finally {
		saving.value = false;
	}
}

onMounted(async () => {
	try {
		fill(await knowledgeModelAccess(props.id));
	} catch {
		store.toast(t('load_error'), {type: 'error'});
	}
});
</script>
