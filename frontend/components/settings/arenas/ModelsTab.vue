<template>
	<div class="space-y-4">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ t('models_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('models_description') }}</p>
		</div>
		<div v-for="(level, index) in form.levels" :key="level.uid" class="flex items-center gap-2" data-testid="arena-model-row">
			<span class="w-6 shrink-0 text-center text-xs text-muted-foreground">{{ index + 1 }}</span>
			<SettingsDefaultModelPicker
				class="flex-1"
				:model-value="level.model_key"
				:placeholder="store.getTranslation('presets.editor.choose_model')"
				:disabled="disabled"
				@update:model-value="value => (level.model_key = value)"
				@selected="model => selectModel(level, model)"
			/>
			<ShadButton
				variant="ghost"
				size="icon"
				class="h-8 w-8 shrink-0"
				:disabled="disabled || form.levels.length <= MIN_PRESET_LEVELS"
				:aria-label="t('remove_model')"
				data-testid="arena-model-remove"
				@click="form.levels.splice(index, 1)"
			>
				<Trash2 class="h-3.5 w-3.5" />
			</ShadButton>
		</div>
		<ShadButton variant="outline" size="sm" :disabled="disabled || form.levels.length >= MAX_PRESET_LEVELS" data-testid="arena-model-add" @click="form.levels.push(levelRow('', null))">
			<Plus class="h-4 w-4" />
			{{ t('add_model') }}
		</ShadButton>
	</div>
</template>

<script setup lang="ts">
import {Plus, Trash2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {ModelList} from '~/types/chat';
import type {LevelRow} from '~/utils/presets';

const props = defineProps<{form: {levels: LevelRow[]}; disabled?: boolean}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`presets.arena.${key}`);

onMounted(() => {
	while (props.form.levels.length < MIN_PRESET_LEVELS) props.form.levels.push(levelRow('', null));
});

function selectModel(level: LevelRow, model: ModelList | null) {
	level.model_id = model?.id ?? '';
	level.model_key = model?.model_id ?? null;
	level.model_name = model?.display_name ?? null;
}
</script>
