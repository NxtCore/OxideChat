<template>
	<div class="w-full flex flex-col lg:max-h-[calc(100dvh-12rem)]">
		<!-- Top Header Bar -->
		<div class="sticky top-0 z-10 bg-background/95 backdrop-blur-sm border-b border-border px-3 py-3 flex items-center gap-3 flex-shrink-0">
			<ShadButton variant="ghost" size="icon" @click="handleBack">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>

			<div class="flex items-center gap-3 flex-1 min-w-0">
				<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-muted flex-shrink-0 overflow-hidden">
					<img v-if="model?.icon" :src="model.icon" class="h-full w-full object-cover" alt="Model icon" />
					<div
						v-else-if="providerIcon?.type === 'svg'"
						v-html="providerIcon.icon"
						class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
					/>
					<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" alt="Provider icon" class="h-5 w-5" />
					<Bot v-else class="h-5 w-5 text-muted-foreground" />
				</div>
				<div class="min-w-0">
					<h2 class="text-sm font-semibold text-foreground truncate">
						{{ model ? model.display_name : store.getTranslation('settings.models.editor.loading') }}
					</h2>
					<p v-if="model" class="text-xs text-muted-foreground truncate">{{ model.provider.name }} &bull; {{ model.model_id }}</p>
				</div>
			</div>

			<span v-if="hasUnsavedChanges" class="hidden sm:flex items-center gap-1.5 text-xs text-amber-500 flex-shrink-0">
				<AlertCircle class="h-3.5 w-3.5" />
				{{ store.getTranslation('settings.models.editor.unsaved_changes') }}
			</span>

			<div v-if="hasUnsavedChanges" class="flex items-center gap-2">
				<ShadButton size="sm" class="gap-1.5" :disabled="saving || loading || !model" @click="saveModel">
					<Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
					<Save v-else class="h-4 w-4" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</div>
		</div>

		<!-- Stepper Navigation -->
		<div v-if="model && !loading" class="border-b border-border px-3 py-3 flex-shrink-0 overflow-auto">
			<nav class="flex items-center justify-center gap-1 sm:gap-2 w-full">
				<button v-for="(step, index) in steps" :key="step.key" class="flex items-center gap-1.5 group" @click="currentStep = index">
					<span
						class="flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium transition-colors"
						:class="
							currentStep === index
								? 'bg-primary text-primary-foreground'
								: currentStep > index
									? 'bg-primary/20 text-primary'
									: 'bg-muted text-muted-foreground'
						"
					>
						<Check v-if="currentStep > index" class="h-3.5 w-3.5" />
						<span v-else>{{ index + 1 }}</span>
					</span>
					<span class="text-xs font-medium transition-colors hidden sm:inline" :class="currentStep === index ? 'text-foreground' : 'text-muted-foreground'">
						{{ store.getTranslation(step.label) }}
					</span>
					<ChevronRight v-if="index < steps.length - 1" class="h-3.5 w-3.5 text-muted-foreground mx-0.5" />
				</button>
			</nav>
		</div>

		<!-- Loading State -->
		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<!-- Not Found State -->
		<div v-else-if="!model" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<p>{{ store.getTranslation('settings.models.not_found') }}</p>
		</div>

		<!-- Step Content -->
		<div v-else class="flex-1 overflow-y-auto px-4 py-5">
			<SettingsModelsGeneralTab v-show="currentStep === 0" :form="formData" :model="model" />
			<SettingsModelsPromptTab v-show="currentStep === 1" :form="formData" />
			<SettingsModelsParametersTab v-show="currentStep === 2" :form="formData" :capabilities="modelCapabilities" />
			<SettingsModelsProvidersTab v-show="currentStep === 3" :model-id="modelId" :active="currentStep === 3" />
		</div>
	</div>

	<!-- Unsaved Changes Dialog -->
	<ShadDialog v-model:open="showUnsavedDialog">
		<ShadDialogContent class="sm:max-w-md">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('settings.models.editor.unsaved_dialog_title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('settings.models.editor.unsaved_dialog_desc') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<ShadDialogFooter class="flex-col-reverse sm:flex-row gap-2 sm:gap-0">
				<ShadButton variant="outline" @click="showUnsavedDialog = false">
					{{ store.getTranslation('settings.models.editor.keep_editing') }}
				</ShadButton>
				<ShadButton variant="destructive" @click="confirmGoBack">
					{{ store.getTranslation('settings.models.editor.discard_changes') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {ArrowLeft, Loader2, Bot, AlertCircle, Save, Check, ChevronRight} from 'lucide-vue-next';
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
const saving = ref(false);
const showUnsavedDialog = ref(false);
const currentStep = ref(0);
const originalData = ref<string>('');

const steps = [
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
});

const hasUnsavedChanges = computed(() => JSON.stringify(formData) !== originalData.value);

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

	originalData.value = JSON.stringify(formData);
}

async function loadModel() {
	loading.value = true;
	try {
		const res = await adminModel(modelId);
		if (res) {
			model.value = res as Record<string, any>;
			populateForm(model.value);
		}
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.not_found'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

function handleBack() {
	if (hasUnsavedChanges.value) {
		showUnsavedDialog.value = true;
	} else {
		router.push('/settings/models');
	}
}

function confirmGoBack() {
	showUnsavedDialog.value = false;
	router.push('/settings/models');
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

	return patch;
}

async function saveModel() {
	if (!model.value) return;
	saving.value = true;
	try {
		const patch = buildPatchPayload();
		if (Object.keys(patch).length === 0) {
			router.push('/settings/models');
			return;
		}

		await patchAdminModel(modelId, patch);

		store.toast(store.getTranslation('settings.models.save_success'), {type: 'success'});
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.save_error'), {type: 'error'});
	} finally {
		saving.value = false;
	}
}
</script>
