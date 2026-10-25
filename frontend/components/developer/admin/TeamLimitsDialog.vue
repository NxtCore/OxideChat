<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-[640px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.admin.teams.limits_title', {name: team?.name ?? ''}) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.admin.teams.limits_description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div class="space-y-3 py-2">
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.admin.teams.concurrency') }}</ShadLabel>
					<ShadInput v-model="concurrency" type="number" min="1" :placeholder="store.getTranslation('developer.admin.detail.no_cap')" />
				</div>
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
					<Plus class="mr-1.5 h-4 w-4" />{{ store.getTranslation('developer.limits.add') }}
				</ShadButton>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.admin.reason') }}</ShadLabel>
					<ShadTextarea v-model="reason" rows="2" maxlength="500" :placeholder="store.getTranslation('developer.admin.reason_placeholder')" />
				</div>
			</div>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :disabled="saving" @click="save">
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
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {errorMessage} from '~/utils/developer';
import type {DeveloperKeyRule} from '~/types/developer';
import type {AdminTeam} from '~/types/gatewayAdmin';

const props = defineProps<{open: boolean; team: AdminTeam | null}>();
const emit = defineEmits<{'update:open': [value: boolean]; saved: [team: AdminTeam]}>();

const store = useMainStore();
const admin = useGatewayAdminStore();
const rules = ref<DeveloperKeyRule[]>([]);
const concurrency = ref('');
const reason = ref('');
const saving = ref(false);

watch(
	() => props.open,
	value => {
		if (!value || !props.team) return;
		rules.value = props.team.rules.map(rule => ({...rule}));
		concurrency.value = props.team.max_concurrent_requests === null ? '' : String(props.team.max_concurrent_requests);
		reason.value = '';
	}
);

function addRule() {
	rules.value.push({route_class: 'inference', metric: 'requests', capacity: 60, refill_period_seconds: 60});
}

async function save() {
	if (!props.team) return;
	saving.value = true;
	try {
		const cap = concurrency.value === '' ? null : Number(concurrency.value);
		const updated = await admin.setTeamLimits(props.team.id, rules.value, cap, reason.value.trim());
		store.toast(store.getTranslation('developer.admin.teams.limits_saved'), {type: 'success'});
		emit('saved', updated);
		emit('update:open', false);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.admin.teams.limits_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}
</script>
