<template>
	<SettingsPage :title="model ? model.display_name : store.getTranslation('settings.models.editor.loading')" :description="model ? `${model.provider.name} • ${model.model_id}` : undefined" :form="settingsForm">
		<template #leading>
			<ShadButton variant="ghost" size="icon" class="shrink-0" data-testid="model-editor-back" @click="router.push('/settings/models')">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>
			<div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted">
				<img v-if="model?.icon" :src="model.icon" class="h-full w-full object-cover" alt="Model icon" />
				<div
					v-else-if="providerIcon?.type === 'svg'"
					v-html="providerIcon.icon"
					class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
				/>
				<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" alt="Provider icon" class="h-5 w-5" />
				<Bot v-else class="h-5 w-5 text-muted-foreground" />
			</div>
		</template>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="!model" class="flex items-center justify-center py-12 text-muted-foreground">
			<p>{{ store.getTranslation('settings.models.not_found') }}</p>
		</div>

		<template v-else>
			<ShadTabs v-model="currentTab" class="mb-5 w-full">
				<ShadTabsList>
					<ShadTabsTrigger v-for="tab in tabs" :key="tab.key" :value="tab.key">{{ store.getTranslation(tab.label) }}</ShadTabsTrigger>
				</ShadTabsList>
			</ShadTabs>

			<SettingsModelsGeneralTab v-show="currentTab === 'general'" :form="formData" :model="model" />
			<SettingsModelsPromptTab v-show="currentTab === 'prompt'" :form="formData" />
			<SettingsModelsParametersTab v-show="currentTab === 'parameters'" :form="formData" :capabilities="modelCapabilities" />
			<SettingsModelsProvidersTab
				v-show="currentTab === 'providers'"
				:model-id="modelId"
				:active="currentTab === 'providers'"
				:form="formData"
				:pricing="pricing"
				@remove-override="removePricingOverride"
			/>
		</template>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ArrowLeft, Loader2, Bot} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {sanitizeSvg} from '@/composables/useMarkdown';

const store = useMainStore();
const iconStore = useIconsStore();
const route = useRoute();
const router = useRouter();

const modelId = route.params.id as string;
const model = ref<Record<string, any> | null>(null);
const loading = ref(true);
const currentTab = ref('general');
const originalData = ref<string>('');
const pricing = ref<Record<string, any> | null>(null);

const tabs = [
	{key: 'general', label: 'settings.models.editor.general_info'},
	{key: 'prompt', label: 'settings.models.editor.core_prompt'},
	{key: 'parameters', label: 'settings.models.editor.parameters'},
	{key: 'providers', label: 'settings.models.editor.providers'},
];

const samplingKeys = [
	'temperature',
	'max_tokens',
	'top_p',
	'top_k',
	'frequency_penalty',
	'presence_penalty',
	'seed',
	'stop',
	'logprobs',
	'top_logprobs',
	'parallel_tool_calls',
] as const;

const formData = reactive({
	display_name: '',
	description: '',
	is_enabled: true,
	system_prompt: '',
	icon: '',
	sampling: Object.fromEntries(samplingKeys.map(key => [key, null])) as Record<(typeof samplingKeys)[number], any>,
	reasoning_effort: null as string | null,
	reasoning_budget_tokens: null as number | null,
	pricing: {input: '0', output: '0', reasoning: '', cache_read: '', cache_write: ''},
});

const settingsForm = useSettingsForm({
	value: () => formData,
	restore: saved => Object.assign(formData, saved),
	savedKey: 'settings.models.save_success',
	errorKey: 'settings.models.save_error',
	save: saveModel,
});

const modelCapabilities = computed<string[]>(() => model.value?.capabilities ?? []);

const providerIcon = computed(() => {
	if (!model.value) return null;
	const icon = iconStore.getProviderIcon(model.value.provider.name, model.value.model_id);
	if (icon?.type === 'svg') return {...icon, icon: sanitizeSvg(icon.icon)};
	return icon;
});

onMounted(loadModel);

function populateForm(data: Record<string, any>) {
	formData.display_name = data.display_name || '';
	formData.description = data.description || '';
	formData.is_enabled = data.is_enabled ?? true;
	formData.system_prompt = data.system_prompt || '';
	formData.icon = data.icon || '';

	const s = data.sampling ?? {};
	for (const key of samplingKeys) formData.sampling[key] = s[key] ?? null;
	formData.sampling.stop = Array.isArray(s.stop) && s.stop.length > 0 ? [...s.stop] : null;

	const es = data.extra_settings ?? {};
	formData.reasoning_effort = es.reasoning_effort ?? null;
	formData.reasoning_budget_tokens = es.reasoning_budget_tokens ?? null;
}

async function loadPricing() {
	try {
		pricing.value = (await modelPricing(modelId)) as Record<string, any>;
		const override = pricing.value?.override_pricing ?? null;
		formData.pricing = {
			input: String(override?.input ?? pricing.value?.effective_input ?? 0),
			output: String(override?.output ?? pricing.value?.effective_output ?? 0),
			reasoning: override?.reasoning != null ? String(override.reasoning) : '',
			cache_read: override?.cache_read != null ? String(override.cache_read) : '',
			cache_write: override?.cache_write != null ? String(override.cache_write) : '',
		};
	} catch (e) {
		console.error(e);
	}
}

function markLoaded() {
	originalData.value = JSON.stringify(formData);
	settingsForm.markSaved();
}

async function loadModel() {
	loading.value = true;
	try {
		const res = await adminModel(modelId);
		if (res) {
			model.value = res as Record<string, any>;
			populateForm(model.value);
			await loadPricing();
			markLoaded();
		}
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.not_found'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

async function removePricingOverride() {
	if (!(await settingsForm.confirmDiscard())) return;
	try {
		await deleteModelPricing(modelId);
		await loadPricing();
		markLoaded();
		store.toast(store.getTranslation('settings.models.editor.pricing_deleted'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.models.save_error'), {type: 'error', description: errorMessage(e, '')});
	}
}

function buildPricingPayload(): Record<string, any> {
	const optional = (value: string): number | undefined => (value.trim() === '' ? undefined : Number(value));
	const prices = {
		input: Number(formData.pricing.input || 0),
		output: Number(formData.pricing.output || 0),
		reasoning: optional(formData.pricing.reasoning),
		cache_read: optional(formData.pricing.cache_read),
		cache_write: optional(formData.pricing.cache_write),
	};
	if (!Object.values(prices).every(price => price === undefined || (Number.isFinite(price) && price >= 0))) {
		throw new Error(store.getTranslation('settings.models.editor.pricing_invalid'));
	}
	return {...prices, tiers: []};
}

function buildPatchPayload(): Record<string, any> {
	const original = JSON.parse(originalData.value) as typeof formData;
	const patch: Record<string, any> = {};

	const scalarKeys = ['display_name', 'description', 'is_enabled', 'system_prompt', 'icon', 'reasoning_effort', 'reasoning_budget_tokens'] as const;
	for (const key of scalarKeys) {
		const cur = formData[key];
		if (JSON.stringify(cur) !== JSON.stringify(original[key])) {
			patch[key] = cur === '' ? null : cur;
		}
	}

	const samplingPatch: Record<string, any> = {};
	for (const key of samplingKeys) {
		if (JSON.stringify(formData.sampling[key]) !== JSON.stringify(original.sampling[key])) {
			samplingPatch[key] = formData.sampling[key];
		}
	}
	if (Object.keys(samplingPatch).length > 0) {
		patch.sampling = samplingPatch;
	}

	if (JSON.stringify(formData.pricing) !== JSON.stringify(original.pricing)) {
		patch.pricing = buildPricingPayload();
	}

	return patch;
}

async function saveModel() {
	await patchAdminModel(modelId, buildPatchPayload());
	model.value = (await adminModel(modelId)) as Record<string, any>;
	populateForm(model.value);
	await loadPricing();
	originalData.value = JSON.stringify(formData);
}
</script>
