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
				<div class="space-y-3">
					<div class="flex items-center gap-3">
						<div class="flex h-12 w-12 items-center justify-center rounded-lg bg-muted overflow-hidden border border-border">
							<img v-if="form.icon" :src="form.icon" class="h-full w-full object-cover" alt="Model icon" />
							<Bot v-else class="h-6 w-6 text-muted-foreground" />
						</div>
						<div class="flex gap-2">
							<ShadButton variant="outline" size="sm" class="gap-1.5" @click="iconInputRef?.click()">
								<Upload class="h-3.5 w-3.5" />
								{{ store.getTranslation('settings.models.editor.upload') }}
							</ShadButton>
							<ShadButton v-if="form.icon" variant="ghost" size="sm" class="gap-1.5 text-destructive" @click="form.icon = ''">
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
						<ShadInput id="icon-url" v-model="form.icon" type="url" :placeholder="store.getTranslation('settings.models.editor.icon_url_placeholder')" />
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Bot, Copy, Trash2, Upload} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{
	form: Record<string, any>;
	model: Record<string, any>;
}>();

const store = useMainStore();
const iconInputRef = ref<HTMLInputElement | null>(null);

function copyModelId() {
	navigator.clipboard.writeText(props.model.model_id);
	store.toast(store.getTranslation('common.copied'), {type: 'success'});
}

function handleIconUpload(event: Event) {
	const target = event.target as HTMLInputElement;
	const file = target.files?.[0];
	if (!file) return;

	const reader = new FileReader();
	reader.onload = e => {
		props.form.icon = e.target?.result as string;
	};
	reader.readAsDataURL(file);
	target.value = '';
}
</script>
