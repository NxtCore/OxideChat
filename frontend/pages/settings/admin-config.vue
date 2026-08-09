<template>
	<div class="max-w-4xl">
		<div class="rounded-lg border border-border bg-card p-6">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.admin_config') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.admin_config.description') }}</p>
			</div>

			<div class="flex items-start justify-between gap-4 rounded-lg border border-border p-4">
				<div class="min-w-0">
					<p class="text-sm font-medium text-foreground">{{ store.getTranslation('settings.admin_config.provider_selector.label') }}</p>
					<p class="text-xs text-muted-foreground mt-0.5">{{ store.getTranslation('settings.admin_config.provider_selector.hint') }}</p>
				</div>
				<Switch :modelValue="enableProviderSelector" :disabled="saving" @update:modelValue="toggleProviderSelector" />
			</div>

			<div class="mt-4 flex items-start justify-between gap-4 rounded-lg border border-border p-4">
				<div class="min-w-0">
					<p class="text-sm font-medium text-foreground">{{ store.getTranslation('admin.tools.mcp.allow_stdio') }}</p>
					<p class="text-xs text-amber-500 mt-0.5">{{ store.getTranslation('admin.tools.mcp.allow_stdio_hint') }}</p>
				</div>
				<Switch :modelValue="allowServerStdioMcp" :disabled="saving" @update:modelValue="toggleAllowStdioMcp" />
			</div>

			<div class="mt-4 rounded-lg border border-border p-4 space-y-2">
				<div class="min-w-0">
					<p class="text-sm font-medium text-foreground">{{ store.getTranslation('settings.admin.default_model') }}</p>
					<p class="text-xs text-muted-foreground mt-0.5">{{ store.getTranslation('settings.admin.default_model_hint') }}</p>
				</div>
				<DefaultModelPicker
					:model-value="defaultModelId"
					:disabled="saving"
					endpoint="/api/v1/admin/models"
					selected-model-endpoint="/api/v1/admin/models"
					value-mode="uuid"
					:placeholder="store.getTranslation('settings.teams.use_global_default')"
					@update:model-value="setDefaultModel"
				/>
			</div>

		</div>
	</div>
</template>

<script setup lang="ts">
import {ref} from 'vue';
import {useMainStore} from '@/stores';
import {Switch} from '@/components/ui/switch';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';

const store = useMainStore();
const {$customFetch} = useNuxtApp();
const saving = ref(false);
const enableProviderSelector = ref(store.base?.enable_provider_selector ?? false);
const allowServerStdioMcp = ref(store.base?.allow_server_stdio_mcp ?? false);
const defaultModelId = ref<string | null>(store.base?.default_model_id ?? null);

async function toggleProviderSelector(val: boolean) {
	const previous = enableProviderSelector.value;
	enableProviderSelector.value = val;
	saving.value = true;
	try {
		await $customFetch('/api/v1/admin/config', {
			method: 'PATCH',
			body: {enable_provider_selector: val},
		});
		if (store.base) store.base.enable_provider_selector = val;
	} catch (error) {
		console.error('Failed to update provider selector setting:', error);
		enableProviderSelector.value = previous;
	} finally {
		saving.value = false;
	}
}

async function toggleAllowStdioMcp(val: boolean) {
	const previous = allowServerStdioMcp.value;
	allowServerStdioMcp.value = val;
	saving.value = true;
	try {
		await $customFetch('/api/v1/admin/config', {
			method: 'PATCH',
			body: {allow_server_stdio_mcp: val},
		});
		if (store.base) store.base.allow_server_stdio_mcp = val;
	} catch (error) {
		console.error('Failed to update stdio MCP setting:', error);
		allowServerStdioMcp.value = previous;
	} finally {
		saving.value = false;
	}
}

async function setDefaultModel(val: string | null) {
	const previous = defaultModelId.value;
	const newId = val;
	defaultModelId.value = newId;
	saving.value = true;
	try {
		await $customFetch('/api/v1/admin/config', {
			method: 'PATCH',
			body: {default_model_id: newId},
		});
		if (store.base) store.base.default_model_id = newId;
	} catch (error) {
		console.error('Failed to update default model:', error);
		defaultModelId.value = previous;
	} finally {
		saving.value = false;
	}
}

</script>
