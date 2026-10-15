<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[440px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.rotate.title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.rotate.description', {name: apiKey?.name ?? ''}) }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div class="space-y-1.5 py-2">
				<ShadLabel>{{ store.getTranslation('developer.rotate.overlap') }}</ShadLabel>
				<ShadSelect v-model="overlap">
					<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem v-for="option in options" :key="option.value" :value="option.value">{{ store.getTranslation(option.label) }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
				<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.rotate.overlap_hint') }}</p>
			</div>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :disabled="saving" @click="submit">
					<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
					{{ store.getTranslation('developer.rotate.submit') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {DeveloperKey, DeveloperKeyRotation} from '~/types/developer';

const props = defineProps<{open: boolean; apiKey: DeveloperKey | null}>();
const emit = defineEmits<{'update:open': [value: boolean]; rotated: [rotation: DeveloperKeyRotation]}>();

const store = useMainStore();
const developer = useDeveloperStore();
const saving = ref(false);
const overlap = ref('86400');

const options = [
	{value: '0', label: 'developer.rotate.overlap_none'},
	{value: '3600', label: 'developer.rotate.overlap_hour'},
	{value: '86400', label: 'developer.rotate.overlap_day'},
	{value: '604800', label: 'developer.rotate.overlap_week'},
];

watch(
	() => props.open,
	(value) => {
		if (value) overlap.value = '86400';
	}
);

async function submit() {
	if (!props.apiKey) return;
	saving.value = true;
	try {
		const rotation = await developer.rotateKey(props.apiKey.id, Number(overlap.value));
		emit('update:open', false);
		emit('rotated', rotation);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.rotate.error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}
</script>
