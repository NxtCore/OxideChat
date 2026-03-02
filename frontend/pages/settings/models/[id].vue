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
					<p v-if="model" class="text-xs text-muted-foreground truncate">{{ model.provider_name }} &bull; {{ model.model_id }}</p>
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
			<!-- Step 1: General Info -->
			<div v-show="currentStep === 0" class="space-y-5">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.general_info') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.general_info_desc') }}
					</p>
				</div>

				<div class="space-y-4">
					<!-- Display Name -->
					<div class="space-y-1.5">
						<ShadLabel for="display-name">
							{{ store.getTranslation('settings.models.editor.display_name') }}
						</ShadLabel>
						<ShadInput
							id="display-name"
							v-model="formData.display_name"
							:placeholder="store.getTranslation('settings.models.editor.display_name_placeholder')"
						/>
					</div>

					<!-- Model ID (Read Only) -->
					<div class="space-y-1.5">
						<ShadLabel>
							{{ store.getTranslation('settings.models.editor.model_id') }}
						</ShadLabel>
						<div class="flex items-center gap-2">
							<ShadInput :model-value="`${model.provider.name} • ${model.model_id}`" disabled class="flex-1 font-mono text-xs" />
							<ShadButton variant="outline" size="icon" class="flex-shrink-0" @click="copyModelId">
								<Copy class="h-3.5 w-3.5" />
							</ShadButton>
						</div>
					</div>

					<!-- Description -->
					<div class="space-y-1.5">
						<ShadLabel for="description">
							{{ store.getTranslation('settings.models.editor.description') }}
						</ShadLabel>
						<ShadTextarea
							id="description"
							v-model="formData.description"
							:placeholder="store.getTranslation('settings.models.editor.description_placeholder')"
							rows="3"
						/>
					</div>

					<!-- Enable Model -->
					<div class="flex items-center justify-between rounded-lg border border-border p-3">
						<div class="space-y-0.5">
							<ShadLabel>
								{{ store.getTranslation('settings.models.editor.enable_model') }}
							</ShadLabel>
							<p class="text-xs text-muted-foreground">
								{{ store.getTranslation('settings.models.editor.enable_model_desc') }}
							</p>
						</div>
						<ShadSwitch v-model:model-value="formData.is_enabled" />
					</div>

					<!-- Change Model Icon -->
					<div class="space-y-1.5">
						<ShadLabel>
							{{ store.getTranslation('settings.models.editor.change_icon') }}
						</ShadLabel>
						<div class="space-y-3">
							<div class="flex items-center gap-3">
								<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-muted overflow-hidden border border-border">
									<img v-if="formData.icon" :src="formData.icon" class="h-full w-full object-cover" alt="Model icon" />
									<Bot v-else class="h-6 w-6 text-muted-foreground" />
								</div>
								<div class="flex gap-2">
									<ShadButton variant="outline" size="sm" class="gap-1.5" @click="triggerIconUpload">
										<Upload class="h-3.5 w-3.5" />
										{{ store.getTranslation('settings.models.editor.upload') }}
									</ShadButton>
									<ShadButton v-if="formData.icon" variant="ghost" size="sm" class="gap-1.5 text-destructive" @click="formData.icon = ''">
										<Trash2 class="h-3.5 w-3.5" />
										{{ store.getTranslation('common.remove') }}
									</ShadButton>
								</div>
								<input ref="iconInputRef" type="file" accept="image/*" class="hidden" @change="handleIconUpload" />
							</div>
							<div class="space-y-1.5">
								<ShadLabel for="icon-url">
									{{ store.getTranslation('settings.models.editor.icon_url') }}
								</ShadLabel>
								<ShadInput
									id="icon-url"
									v-model="formData.icon"
									type="url"
									:placeholder="store.getTranslation('settings.models.editor.icon_url_placeholder')"
								/>
							</div>
						</div>
					</div>
				</div>
			</div>

			<!-- Step 2: Core Prompt -->
			<div v-show="currentStep === 1" class="space-y-5">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.core_prompt') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.core_prompt_desc') }}
					</p>
				</div>

				<div class="space-y-4">
					<!-- System Prompt -->
					<div class="space-y-1.5">
						<div class="flex items-center justify-between">
							<ShadLabel for="system-prompt">
								{{ store.getTranslation('settings.models.editor.system_prompt') }}
							</ShadLabel>
							<span class="text-xs text-muted-foreground">
								{{ formData.system_prompt?.length || 0 }} {{ store.getTranslation('settings.models.editor.characters') }}
							</span>
						</div>
						<ShadTextarea
							id="system-prompt"
							v-model="formData.system_prompt"
							:placeholder="store.getTranslation('settings.models.editor.system_prompt_placeholder')"
							rows="12"
							class="font-mono text-sm"
						/>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('settings.models.editor.system_prompt_hint') }}
						</p>
					</div>
				</div>
			</div>

			<!-- Step 3: Parameters (Sampling) -->
			<div v-show="currentStep === 2" class="space-y-5">
				<div>
					<h3 class="text-base font-semibold text-foreground mb-1">
						{{ store.getTranslation('settings.models.editor.parameters') }}
					</h3>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.parameters_desc') }}
					</p>
				</div>

				<div class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-5">
					<!-- Temperature -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.temperature') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.temperature_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput v-model.number="formData.sampling.temperature" type="number" min="0" max="2" step="0.1" class="w-20 h-8 text-xs text-right" />
						</div>
						<ShadSlider v-model="temperatureSlider" :min="0" :max="2" :step="0.1" class="w-full" />
						<div class="flex justify-between text-[10px] text-muted-foreground">
							<span>0</span>
							<span>1</span>
							<span>2</span>
						</div>
					</div>

					<!-- Max Tokens -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.max_tokens') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.max_tokens_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput v-model.number="formData.sampling.max_tokens" type="number" min="1" max="128000" step="1" class="w-24 h-8 text-xs text-right" />
						</div>
					</div>

					<!-- Top-P -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.top_p') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.top_p_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput v-model.number="formData.sampling.top_p" type="number" min="0" max="1" step="0.05" class="w-20 h-8 text-xs text-right" />
						</div>
						<ShadSlider v-model="topPSlider" :min="0" :max="1" :step="0.05" class="w-full" />
						<div class="flex justify-between text-[10px] text-muted-foreground">
							<span>0</span>
							<span>0.5</span>
							<span>1</span>
						</div>
					</div>

					<!-- Top-K -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.top_k') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.top_k_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput v-model.number="formData.sampling.top_k" type="number" min="0" max="500" step="1" class="w-24 h-8 text-xs text-right" />
						</div>
					</div>

					<!-- Frequency Penalty -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.frequency_penalty') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.frequency_penalty_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput
								v-model.number="formData.sampling.frequency_penalty"
								type="number"
								min="0"
								max="2"
								step="0.1"
								class="w-20 h-8 text-xs text-right"
							/>
						</div>
						<ShadSlider v-model="frequencyPenaltySlider" :min="0" :max="2" :step="0.1" class="w-full" />
						<div class="flex justify-between text-[10px] text-muted-foreground">
							<span>0</span>
							<span>1</span>
							<span>2</span>
						</div>
					</div>

					<!-- Presence Penalty -->
					<div class="space-y-2.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<ShadLabel>{{ store.getTranslation('settings.models.editor.presence_penalty') }}</ShadLabel>
								<ShadTooltipProvider>
									<ShadTooltip>
										<ShadTooltipTrigger as-child>
											<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
										</ShadTooltipTrigger>
										<ShadTooltipContent side="top" class="max-w-xs text-xs">
											{{ store.getTranslation('settings.models.editor.presence_penalty_tooltip') }}
										</ShadTooltipContent>
									</ShadTooltip>
								</ShadTooltipProvider>
							</div>
							<ShadInput v-model.number="formData.sampling.presence_penalty" type="number" min="0" max="2" step="0.1" class="w-20 h-8 text-xs text-right" />
						</div>
						<ShadSlider v-model="presencePenaltySlider" :min="0" :max="2" :step="0.1" class="w-full" />
						<div class="flex justify-between text-[10px] text-muted-foreground">
							<span>0</span>
							<span>1</span>
							<span>2</span>
						</div>
					</div>
				</div>
			</div>
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
import {ArrowLeft, Loader2, Bot, AlertCircle, Save, Check, ChevronRight, ChevronLeft, Copy, Upload, Trash2, Info} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useNuxtApp} from '#app';

const store = useMainStore();
const iconStore = useIconsStore();
const route = useRoute();
const router = useRouter();
const {$customFetch} = useNuxtApp();

const modelId = route.params.id as string;
const model = ref<Record<string, any> | null>(null);
const loading = ref(true);
const saving = ref(false);
const showUnsavedDialog = ref(false);
const currentStep = ref(0);
const iconInputRef = ref<HTMLInputElement | null>(null);

const originalData = ref<string>('');

const steps = [
	{key: 'general', label: 'settings.models.editor.general_info'},
	{key: 'prompt', label: 'settings.models.editor.core_prompt'},
	{key: 'parameters', label: 'settings.models.editor.parameters'},
];

const formData = reactive({
	display_name: '',
	description: '',
	is_enabled: true,
	system_prompt: '',
	icon: '',
	sampling: {
		temperature: 0.7,
		max_tokens: 4096,
		top_p: 1,
		top_k: 0,
		frequency_penalty: 0,
		presence_penalty: 0,
	},
});

const temperatureSlider = computed({
	get: () => [formData.sampling.temperature],
	set: (val: number[]) => {
		formData.sampling.temperature = val[0];
	},
});

const topPSlider = computed({
	get: () => [formData.sampling.top_p],
	set: (val: number[]) => {
		formData.sampling.top_p = val[0];
	},
});

const frequencyPenaltySlider = computed({
	get: () => [formData.sampling.frequency_penalty],
	set: (val: number[]) => {
		formData.sampling.frequency_penalty = val[0];
	},
});

const presencePenaltySlider = computed({
	get: () => [formData.sampling.presence_penalty],
	set: (val: number[]) => {
		formData.sampling.presence_penalty = val[0];
	},
});

const hasUnsavedChanges = computed(() => {
	return JSON.stringify(formData) !== originalData.value;
});

const providerIcon = computed(() => {
	if (!model.value) return null;
	return iconStore.getProviderIcon(model.value.provider_name, model.value.model_id);
});

onMounted(async () => {
	await loadModel();
});

function populateForm(data: Record<string, any>) {
	formData.display_name = data.display_name || '';
	formData.description = data.description || '';
	formData.is_enabled = data.is_enabled ?? true;
	formData.system_prompt = data.system_prompt || '';
	formData.icon = data.icon || '';
	formData.sampling.temperature = data.sampling?.temperature ?? 0.7;
	formData.sampling.max_tokens = data.sampling?.max_tokens ?? 4096;
	formData.sampling.top_p = data.sampling?.top_p ?? 1;
	formData.sampling.top_k = data.sampling?.top_k ?? 0;
	formData.sampling.frequency_penalty = data.sampling?.frequency_penalty ?? 0;
	formData.sampling.presence_penalty = data.sampling?.presence_penalty ?? 0;

	originalData.value = JSON.stringify(formData);
}

async function loadModel() {
	loading.value = true;
	try {
		const res = await $customFetch(`/api/v1/admin/models/${modelId}`);
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

function copyModelId() {
	if (!model.value) return;
	navigator.clipboard.writeText(model.value.model_id);
	store.toast(store.getTranslation('common.copied'), {type: 'success'});
}

function triggerIconUpload() {
	iconInputRef.value?.click();
}

function handleIconUpload(event: Event) {
	const target = event.target as HTMLInputElement;
	const file = target.files?.[0];
	if (!file) return;

	const reader = new FileReader();
	reader.onload = e => {
		formData.icon = e.target?.result as string;
	};
	reader.readAsDataURL(file);
	target.value = '';
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

async function saveModel() {
	if (!model.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/models/${modelId}`, {
			method: 'PUT',
			body: {
				display_name: formData.display_name,
				is_enabled: formData.is_enabled,
				system_prompt: formData.system_prompt,
				sampling: formData.sampling,
				icon: formData.icon,
				description: formData.description,
			},
		});

		store.toast(store.getTranslation('settings.models.save_success'), {type: 'success'});
		router.push('/settings/models');
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.save_error'), {type: 'error'});
	} finally {
		saving.value = false;
	}
}
</script>
