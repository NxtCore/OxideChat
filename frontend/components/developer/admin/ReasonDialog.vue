<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[480px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ title }}</ShadDialogTitle>
				<ShadDialogDescription>{{ description }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div class="space-y-1.5 py-2">
				<ShadLabel>{{ store.getTranslation('developer.admin.reason') }}</ShadLabel>
				<ShadTextarea v-model="reason" rows="3" maxlength="500" :placeholder="store.getTranslation('developer.admin.reason_placeholder')" />
			</div>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :variant="destructive ? 'destructive' : 'default'" :disabled="busy" @click="emit('confirm', reason.trim())">
					<Loader2 v-if="busy" class="mr-2 h-4 w-4 animate-spin" />
					{{ confirmLabel }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{open: boolean; title: string; description: string; confirmLabel: string; destructive?: boolean; busy?: boolean}>();
const emit = defineEmits<{'update:open': [value: boolean]; confirm: [reason: string]}>();

const store = useMainStore();
const reason = ref('');

watch(
	() => props.open,
	value => {
		if (value) reason.value = '';
	}
);
</script>
