<template>
	<div class="space-y-6">
		<div class="space-y-4 rounded-lg border border-border bg-card p-6">
			<h3 class="text-lg font-medium">{{ store.getTranslation('settings.models.editor.general') }}</h3>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-2">
					<ShadLabel for="display-name">{{ store.getTranslation('settings.models.editor.display_name') }}</ShadLabel>
					<ShadInput id="display-name" v-model="form.display_name" />
				</div>
				<div class="space-y-2 flex flex-col justify-end">
					<div class="flex items-center space-x-2 h-10">
						<ShadSwitch id="is-enabled" :modelValue="form.is_enabled" @update:checked="v => (form.is_enabled = v)" />
						<ShadLabel for="is-enabled">{{ store.getTranslation('settings.models.editor.is_enabled') }}</ShadLabel>
					</div>
				</div>
			</div>
		</div>

		<div class="space-y-4 rounded-lg border border-border bg-card p-6">
			<h3 class="text-lg font-medium">{{ store.getTranslation('settings.models.editor.icon_url') }}</h3>
			<div class="flex items-start gap-5">
				<div class="flex h-16 w-16 shrink-0 items-center justify-center rounded-xl bg-muted border border-border overflow-hidden">
					<img v-if="form.icon" :src="form.icon" class="h-10 w-10 object-cover rounded" alt="Model icon" @error="form.icon = ''" />
					<div
						v-else-if="providerIcon?.type === 'svg'"
						v-html="providerIcon.icon"
						class="h-8 w-8 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
					/>
					<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" alt="Provider icon" class="h-8 w-8" />
					<Bot v-else class="h-8 w-8 text-muted-foreground" />
				</div>

				<div class="flex-1 min-w-0">
					<ShadTabs v-model="iconInputMode" class="w-full">
						<ShadTabsList class="grid grid-cols-2 w-40 mb-3">
							<ShadTabsTrigger value="url">{{ store.getTranslation('settings.models.editor.icon_tab_url') }}</ShadTabsTrigger>
							<ShadTabsTrigger value="upload">{{ store.getTranslation('settings.models.editor.icon_tab_upload') }}</ShadTabsTrigger>
						</ShadTabsList>
						<ShadTabsContent value="url">
							<div class="flex gap-2">
								<ShadInput v-model="form.icon" placeholder="https://example.com/icon.png" class="flex-1" />
								<ShadButton
									v-if="form.icon"
									variant="ghost"
									size="icon"
									:title="store.getTranslation('settings.models.editor.icon_clear')"
									@click="form.icon = ''"
								>
									<X class="h-4 w-4" />
								</ShadButton>
							</div>
						</ShadTabsContent>
						<ShadTabsContent value="upload">
							<div class="space-y-3">
								<input ref="iconFileInputRef" type="file" accept="image/*" class="hidden" @change="handleIconUpload" />
								<ShadButton variant="outline" size="sm" class="gap-2" @click="iconFileInputRef?.click()">
									<Upload class="h-4 w-4" />
									{{ store.getTranslation('settings.models.editor.icon_choose_file') }}
								</ShadButton>
								<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.models.editor.icon_upload_hint') }}</p>
							</div>
						</ShadTabsContent>
					</ShadTabs>
				</div>
			</div>
		</div>

		<div class="space-y-4 rounded-lg border border-border bg-card p-6">
			<h3 class="text-lg font-medium">{{ store.getTranslation('settings.models.editor.description') }}</h3>
			<ShadTabs v-model="descriptionTab" class="w-full">
				<ShadTabsList class="grid grid-cols-2 w-44 mb-1">
					<ShadTabsTrigger value="write">{{ store.getTranslation('settings.models.editor.description_tab_write') }}</ShadTabsTrigger>
					<ShadTabsTrigger value="preview">{{ store.getTranslation('settings.models.editor.description_tab_preview') }}</ShadTabsTrigger>
				</ShadTabsList>
				<ShadTabsContent value="write" class="mt-3">
					<ShadTextarea
						v-model="form.description"
						class="min-h-[180px] font-mono text-sm"
						:placeholder="store.getTranslation('settings.models.editor.description_placeholder')"
					/>
					<p class="text-xs text-muted-foreground mt-1.5 flex items-center gap-1">
						<FileText class="h-3 w-3" />
						{{ store.getTranslation('settings.models.editor.description_markdown_hint') }}
					</p>
				</ShadTabsContent>
				<ShadTabsContent value="preview" class="mt-3">
					<div class="min-h-[180px] rounded-md border border-border bg-background/50 p-4">
						<div v-if="isRenderingPreview" class="flex items-center justify-center py-8 text-muted-foreground">
							<Loader2 class="h-5 w-5 animate-spin" />
						</div>
						<div v-else-if="previewHtml" class="prose prose-sm dark:prose-invert max-w-none" v-html="previewHtml" />
						<p v-else class="text-sm text-muted-foreground/60 italic">
							{{ store.getTranslation('settings.models.editor.description_empty_preview') }}
						</p>
					</div>
				</ShadTabsContent>
			</ShadTabs>
		</div>

		<div class="space-y-4 rounded-lg border border-border bg-card p-6">
			<h3 class="text-lg font-medium">{{ store.getTranslation('settings.models.editor.system_prompt') }}</h3>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.models.editor.system_prompt_desc') }}</p>
			<ShadTextarea
				v-model="form.system_prompt"
				class="min-h-[150px] font-mono text-sm"
				:placeholder="store.getTranslation('settings.models.editor.system_prompt_placeholder')"
			/>
		</div>

		<div class="space-y-4 rounded-lg border border-border bg-card p-6">
			<div>
				<h3 class="text-lg font-medium">{{ store.getTranslation('settings.models.editor.sampling') }}</h3>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.models.editor.sampling_desc') }}</p>
			</div>

			<div class="flex flex-col gap-3 pt-2">
				<div
					v-for="setting in samplingConfig"
					:key="setting.id"
					class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 rounded-md border border-border/60 bg-muted/10 p-4 transition-colors hover:bg-muted/20"
				>
					<ShadLabel :for="setting.id" class="text-sm font-medium cursor-pointer">
						{{ store.getTranslation(setting.translationKey) }}
					</ShadLabel>
					<ShadInput
						:id="setting.id"
						type="number"
						:step="setting.step"
						v-model.number="form.sampling[setting.id]"
						placeholder="Default"
						class="w-full sm:w-40 bg-background"
					/>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ref, watch, computed} from 'vue';
import {Bot, Loader2, X, Upload, FileText} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useMarkdown} from '@/composables/useMarkdown';

const props = defineProps({
	modelValue: {
		type: Object,
		required: true,
	},
});

const store = useMainStore();
const iconStore = useIconsStore();
const {renderComplete} = useMarkdown();

const form = ref(JSON.parse(JSON.stringify(props.modelValue)));
if (!form.value.sampling) form.value.sampling = {};

const originalSnapshot = ref(JSON.stringify(form.value));
const iconInputMode = ref('url');
const descriptionTab = ref('write');
const previewHtml = ref('');
const isRenderingPreview = ref(false);
const iconFileInputRef = ref<HTMLInputElement | null>(null);
const samplingConfig = [
	{
		id: 'temperature',
		translationKey: 'settings.models.editor.temperature',
		step: '0.1',
	},
	{
		id: 'top_p',
		translationKey: 'settings.models.editor.top_p',
		step: '0.05',
	},
	{
		id: 'max_tokens',
		translationKey: 'settings.models.editor.max_tokens',
		step: '1',
	},
];

const providerIcon = computed(() => {
	if (!form.value.provider_name) return null;
	return iconStore.getProviderIcon(form.value.provider_name, form.value.model_id);
});

const isDirty = computed(() => JSON.stringify(form.value) !== originalSnapshot.value);

watch(
	() => props.modelValue,
	newVal => {
		form.value = JSON.parse(JSON.stringify(newVal));
		if (!form.value.sampling) form.value.sampling = {};
		originalSnapshot.value = JSON.stringify(form.value);
	},
	{deep: true}
);

watch(descriptionTab, async tab => {
	if (tab === 'preview') {
		isRenderingPreview.value = true;
		previewHtml.value = await renderComplete(form.value.description || '');
		isRenderingPreview.value = false;
	}
});

function handleIconUpload(e: Event) {
	const file = (e.target as HTMLInputElement).files?.[0];
	if (!file) return;
	const reader = new FileReader();
	reader.onload = event => {
		form.value.icon = event.target?.result as string;
	};
	reader.readAsDataURL(file);
	if (iconFileInputRef.value) iconFileInputRef.value.value = '';
}

function getFormData() {
	return form.value;
}
defineExpose({isDirty, getFormData});
</script>
