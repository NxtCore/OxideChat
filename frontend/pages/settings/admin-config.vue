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
		</div>
	</div>
</template>

<script setup lang="ts">
import {ref} from 'vue';
import {useMainStore} from '@/stores';
import {Switch} from '@/components/ui/switch';

const store = useMainStore();
const saving = ref(false);
const enableProviderSelector = ref(store.base?.enable_provider_selector ?? false);

async function toggleProviderSelector(val: boolean) {
	const previous = enableProviderSelector.value;
	enableProviderSelector.value = val;
	saving.value = true;
	try {
		const {$customFetch} = useNuxtApp();
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
</script>
