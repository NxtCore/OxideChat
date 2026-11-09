<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">
				{{ store.getTranslation('settings.models.editor.general_info') }}
			</h3>
			<p class="text-xs text-muted-foreground">
				{{ store.getTranslation('settings.models.editor.general_info_desc') }}
			</p>
		</div>

		<div class="space-y-4">
			<div class="space-y-1.5">
				<ShadLabel for="display-name">
					{{ store.getTranslation('settings.models.editor.display_name') }}
				</ShadLabel>
				<ShadInput id="display-name" v-model="form.display_name" :placeholder="store.getTranslation('settings.models.editor.display_name_placeholder')" />
			</div>

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

			<div class="space-y-1.5">
				<ShadLabel for="description">
					{{ store.getTranslation('settings.models.editor.description') }}
				</ShadLabel>
				<ShadTextarea id="description" v-model="form.description" :placeholder="store.getTranslation('settings.models.editor.description_placeholder')" rows="3" />
			</div>

			<div class="flex items-center justify-between rounded-lg border border-border p-3">
				<div class="space-y-0.5">
					<ShadLabel>
						{{ store.getTranslation('settings.models.editor.enable_model') }}
					</ShadLabel>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('settings.models.editor.enable_model_desc') }}
					</p>
				</div>
				<ShadSwitch v-model:model-value="form.is_enabled" />
			</div>

			<div class="space-y-1.5">
				<ShadLabel>
					{{ store.getTranslation('settings.models.editor.change_icon') }}
				</ShadLabel>
				<ImagePicker v-model="form.icon" test-id="model-icon">
					<template #fallback><Bot class="h-1/2 w-1/2 text-muted-foreground" /></template>
				</ImagePicker>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Bot, Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{
	form: Record<string, any>;
	model: Record<string, any>;
}>();

const store = useMainStore();

function copyModelId() {
	navigator.clipboard.writeText(props.model.model_id);
	store.toast(store.getTranslation('common.copied'), {type: 'success'});
}
</script>
