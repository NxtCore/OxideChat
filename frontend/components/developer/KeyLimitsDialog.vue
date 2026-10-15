<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-[600px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.limits.title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.limits.description', {name: apiKey?.name ?? ''}) }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div v-if="loading" class="flex justify-center py-8 text-muted-foreground"><Loader2 class="h-5 w-5 animate-spin" /></div>
			<div v-else class="space-y-3 py-2">
				<p v-if="!rules.length" class="rounded-md border border-dashed border-border p-4 text-center text-sm text-muted-foreground">
					{{ store.getTranslation('developer.limits.empty') }}
				</p>
				<div v-for="(rule, index) in rules" :key="index" class="grid grid-cols-2 gap-2 rounded-md border border-border p-3 sm:grid-cols-[1fr_1fr_1fr_1fr_auto]">
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('developer.limits.route_class') }}</ShadLabel>
						<ShadSelect v-model="rule.route_class">
							<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem value="inference">{{ store.getTranslation('developer.limits.class_inference') }}</ShadSelectItem>
								<ShadSelectItem value="utility">{{ store.getTranslation('developer.limits.class_utility') }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('developer.limits.metric') }}</ShadLabel>
						<ShadSelect v-model="rule.metric">
							<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem value="requests">{{ store.getTranslation('developer.limits.metric_requests') }}</ShadSelectItem>
								<ShadSelectItem value="tokens">{{ store.getTranslation('developer.limits.metric_tokens') }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('developer.limits.capacity') }}</ShadLabel>
						<ShadInput v-model.number="rule.capacity" type="number" min="1" />
					</div>
					<div class="space-y-1">
						<ShadLabel class="text-xs">{{ store.getTranslation('developer.limits.period') }}</ShadLabel>
						<ShadInput v-model.number="rule.refill_period_seconds" type="number" min="1" />
					</div>
					<div class="flex items-end">
						<ShadButton variant="ghost" size="icon" :aria-label="store.getTranslation('common.remove')" @click="rules.splice(index, 1)">
							<Trash2 class="h-4 w-4" />
						</ShadButton>
					</div>
				</div>
				<ShadButton variant="outline" size="sm" @click="addRule">
					<Plus class="mr-1.5 h-4 w-4" />
					{{ store.getTranslation('developer.limits.add') }}
				</ShadButton>
			</div>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :disabled="loading || saving" @click="save">
					<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2, Plus, Trash2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {DeveloperKey, DeveloperKeyRule} from '~/types/developer';

const props = defineProps<{open: boolean; apiKey: DeveloperKey | null}>();
const emit = defineEmits<{'update:open': [value: boolean]}>();

const store = useMainStore();
const developer = useDeveloperStore();
const rules = ref<DeveloperKeyRule[]>([]);
const loading = ref(false);
const saving = ref(false);

watch(
	() => props.open,
	async (value) => {
		if (!value || !props.apiKey) return;
		loading.value = true;
		rules.value = [];
		try {
			rules.value = await developer.getLimits(props.apiKey.id);
		} catch (e: any) {
			store.toast(store.getTranslation('developer.limits.load_error'), {type: 'error', description: errorMessage(e, '')});
			emit('update:open', false);
		} finally {
			loading.value = false;
		}
	}
);

function addRule() {
	rules.value.push({route_class: 'inference', metric: 'requests', capacity: 60, refill_period_seconds: 60});
}

async function save() {
	if (!props.apiKey) return;
	saving.value = true;
	try {
		await developer.setLimits(props.apiKey.id, rules.value);
		store.toast(store.getTranslation('developer.limits.saved'), {type: 'success'});
		emit('update:open', false);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.limits.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}
</script>
