<template>
	<ShadDialog :open="!!preset" @update:open="open => !open && emit('close')">
		<ShadDialogContent>
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('presets.delete.title', {name: preset?.name ?? ''}) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('presets.delete.description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('close')">{{ store.getTranslation('presets.delete.cancel') }}</ShadButton>
				<ShadButton variant="destructive" data-testid="preset-delete-confirm" :disabled="deleting" @click="confirm">
					<Loader2 v-if="deleting" class="h-4 w-4 animate-spin" />
					{{ store.getTranslation('presets.delete.confirm') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePresetStore} from '~/stores/presetStore';
import type {PresetResponse} from '~/types/presets';

const props = defineProps<{preset: PresetResponse | null}>();
const emit = defineEmits<{close: []; deleted: []}>();

const store = useMainStore();
const presetStore = usePresetStore();
const deleting = ref(false);

async function confirm() {
	if (!props.preset) return;
	deleting.value = true;
	try {
		await presetStore.deletePreset(props.preset.id);
		store.toast(store.getTranslation('presets.delete.done'), {type: 'success'});
		emit('deleted');
		emit('close');
	} catch (e: any) {
		store.toast(store.getTranslation('presets.delete.error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		deleting.value = false;
	}
}
</script>
