<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ t('tools_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('tools_description') }}</p>
		</div>

		<p v-if="noToolCapability" class="flex items-start gap-2 text-xs text-amber-600">
			<AlertTriangle class="mt-0.5 h-3.5 w-3.5 shrink-0" />
			{{ t('model_no_tools') }}
		</p>

		<div v-if="!preferencesStore.toolsLoaded" class="flex justify-center py-6 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<p v-else-if="tools.length === 0" class="text-sm text-muted-foreground">{{ t('tools_empty') }}</p>
		<div v-else class="grid grid-cols-1 gap-2 sm:grid-cols-2">
			<label v-for="tool in tools" :key="tool.id" class="flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3 hover:bg-accent/30" data-testid="preset-tool">
				<ShadCheckbox :model-value="form.tool_ids.includes(tool.id)" class="mt-0.5" @update:model-value="checked => toggle(tool.id, Boolean(checked))" />
				<div class="min-w-0">
					<p class="text-sm font-medium text-foreground">{{ tool.display_name || tool.name }}</p>
					<p v-if="tool.description" class="line-clamp-2 text-xs text-muted-foreground">{{ tool.description }}</p>
				</div>
			</label>
		</div>

		<div class="flex items-center justify-between rounded-lg border border-border p-3">
			<div class="space-y-0.5">
				<ShadLabel>{{ t('tools_locked') }}</ShadLabel>
				<p class="text-xs text-muted-foreground">{{ t('tools_locked_description') }}</p>
			</div>
			<ShadSwitch v-model:model-value="form.tools_locked" data-testid="preset-tools-locked" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, Loader2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePreferencesStore} from '~/stores/preferencesStore';

const props = defineProps<{
	form: Record<string, any>;
	capabilities: string[] | null;
}>();

const store = useMainStore();
const preferencesStore = usePreferencesStore();
const t = (key: string) => store.getTranslation(`presets.editor.${key}`);

const tools = computed(() => preferencesStore.userTools.filter(tool => tool.is_enabled || props.form.tool_ids.includes(tool.id)));
const noToolCapability = computed(() => props.form.tool_ids.length > 0 && !!props.capabilities && !props.capabilities.includes('TOOLS'));

onMounted(() => preferencesStore.ensureTools());

function toggle(id: string, checked: boolean) {
	props.form.tool_ids = checked ? [...new Set([...props.form.tool_ids, id])] : props.form.tool_ids.filter((toolId: string) => toolId !== id);
}
</script>
