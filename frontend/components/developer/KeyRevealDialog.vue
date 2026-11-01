<template>
	<ShadDialog :open="open" @update:open="onOpenChange">
		<ShadDialogContent class="sm:max-w-[520px]" @interact-outside.prevent @escape-key-down.prevent>
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation(title) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.reveal.description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div class="space-y-4 py-2">
				<div class="flex items-start gap-2 rounded-md border border-border bg-muted/40 p-3">
					<code class="min-w-0 flex-1 break-all font-mono text-sm text-foreground" data-testid="revealed-token">{{ token }}</code>
					<ShadButton variant="outline" size="sm" class="shrink-0" @click="copy">
						<component :is="copied ? Check : Copy" class="mr-1.5 h-4 w-4" />
						{{ store.getTranslation(copied ? 'common.copied' : 'developer.reveal.copy') }}
					</ShadButton>
				</div>
				<p v-if="previousExpiresAt" class="rounded-md border border-border bg-muted/30 p-3 text-sm text-muted-foreground">
					{{ store.getTranslation('developer.reveal.old_key_expires', {date: formatDateTime(previousExpiresAt)}) }}
				</p>
				<DeveloperEndpoints />
				<div class="flex items-start gap-3">
					<ShadCheckbox id="reveal-confirm" v-model="confirmed" class="mt-0.5" />
					<ShadLabel for="reveal-confirm" class="cursor-pointer leading-snug">{{ store.getTranslation('developer.reveal.confirm') }}</ShadLabel>
				</div>
			</div>
			<ShadDialogFooter>
				<ShadButton :disabled="!confirmed" @click="emit('update:open', false)">{{ store.getTranslation('common.close') }}</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Check, Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{
	open: boolean;
	token: string;
	title: string;
	previousExpiresAt?: string | null;
}>();
const emit = defineEmits<{'update:open': [value: boolean]}>();

const store = useMainStore();
const confirmed = ref(false);
const copied = ref(false);

watch(
	() => props.open,
	(value) => {
		if (value) {
			confirmed.value = false;
			copied.value = false;
		}
	}
);

function onOpenChange(value: boolean) {
	if (value || confirmed.value) emit('update:open', value);
}

async function copy() {
	await navigator.clipboard.writeText(props.token);
	copied.value = true;
}
</script>
