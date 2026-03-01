<template>
	<div class="max-w-4xl flex flex-col lg:max-h-[calc(100dvh-12rem)]">
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

			<ShadButton size="sm" class="gap-1.5 flex-shrink-0" :disabled="saving || loading || !model" @click="saveModel">
				<Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
				<Save v-else class="h-4 w-4" />
				{{ store.getTranslation('common.save') }}
			</ShadButton>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>
		<div v-else-if="!model" class="flex items-center justify-center py-12 text-muted-foreground flex-1">
			<p>{{ store.getTranslation('settings.models.not_found') }}</p>
		</div>
		<div v-else class="flex-1 overflow-y-auto px-3 py-4">
			<ModelEditor ref="editorRef" :model-value="model" />
		</div>
	</div>

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
import {ArrowLeft, Loader2, Bot, AlertCircle, Save} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useNuxtApp} from '#app';

import ModelEditor from '@/components/settings/ModelEditor.vue';

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
const editorRef = ref<InstanceType<typeof ModelEditor> | null>(null);

const hasUnsavedChanges = computed(() => editorRef.value?.isDirty ?? false);

const providerIcon = computed(() => {
	if (!model.value) return null;
	return iconStore.getProviderIcon(model.value.provider_name, model.value.model_id);
});

onMounted(async () => {
	await loadModel();
});

async function loadModel() {
	loading.value = true;
	try {
		const res = await $customFetch(`/api/v1/admin/models/${modelId}`);
		if (res) {
			model.value = res as Record<string, any>;
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

async function saveModel() {
	if (!model.value || !editorRef.value) return;
	saving.value = true;
	const formData = editorRef.value.getFormData();
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
