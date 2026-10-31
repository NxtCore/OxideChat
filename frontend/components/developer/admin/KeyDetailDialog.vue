<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-[720px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.admin.detail.title', {name: detail?.key.name ?? ''}) }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.admin.detail.description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div v-if="loading" class="flex justify-center py-8 text-muted-foreground"><Loader2 class="h-5 w-5 animate-spin" /></div>
			<div v-else-if="detail" class="space-y-5 py-2 text-sm">
				<section class="grid grid-cols-2 gap-3 sm:grid-cols-3">
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.owner') }}</p>
						<p class="text-foreground">{{ detail.key.owner_username ?? store.getTranslation('developer.service.badge') }}</p>
					</div>
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.team') }}</p>
						<p class="text-foreground">{{ detail.key.team_name }}</p>
					</div>
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.key') }}</p>
						<p class="font-mono text-foreground">{{ detail.key.key_prefix }}…{{ detail.key.last_four }}</p>
					</div>
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.status') }}</p>
						<ShadBadge variant="secondary">{{ store.getTranslation(`developer.status.${detail.key.status}`) }}</ShadBadge>
					</div>
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.expires') }}</p>
						<p class="text-foreground">{{ formatDate(detail.key.expires_at) }}</p>
					</div>
					<div>
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.keys.last_used') }}</p>
						<p class="text-foreground">{{ formatDate(detail.key.last_used_at) }}</p>
					</div>
				</section>

				<section class="space-y-2">
					<h3 class="font-medium text-foreground">{{ store.getTranslation('developer.admin.detail.limits') }}</h3>
					<p v-if="!detail.team_gateway_enabled" class="text-destructive">{{ store.getTranslation('developer.admin.detail.team_disabled') }}</p>
					<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
						<div class="rounded-md border border-border p-3">
							<p class="mb-1 text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.team_limits') }}</p>
							<p class="text-foreground">{{ concurrency(detail.team_max_concurrent_requests) }}</p>
							<p v-for="rule in detail.team_rules" :key="rule.id" class="text-muted-foreground">{{ ruleText(rule) }}</p>
						</div>
						<div class="rounded-md border border-border p-3">
							<p class="mb-1 text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.key_limits') }}</p>
							<p class="text-foreground">{{ concurrency(detail.key.max_concurrent_requests) }}</p>
							<p v-for="rule in detail.key_rules" :key="rule.id" class="text-muted-foreground">{{ ruleText(rule) }}</p>
						</div>
					</div>
				</section>

				<section class="space-y-2">
					<h3 class="font-medium text-foreground">{{ store.getTranslation('developer.admin.detail.budgets') }}</h3>
					<p v-if="!detail.budgets.length" class="text-muted-foreground">{{ store.getTranslation('developer.admin.detail.no_budgets') }}</p>
					<p v-for="budget in detail.budgets" :key="budget.budget.name" class="text-foreground">
						{{ budget.budget.name }}: {{ formatMoney(budget.spent) }} / {{ formatMoney(budget.amount) }}
						<span v-if="budget.exhausted" class="text-destructive">({{ store.getTranslation('developer.admin.detail.exhausted') }})</span>
					</p>
				</section>

				<section class="space-y-2">
					<h3 class="font-medium text-foreground">{{ store.getTranslation('developer.admin.detail.usage') }}</h3>
					<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
						<div>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.requests') }}</p>
							<p class="tabular-nums text-foreground">{{ detail.usage.total_requests.toLocaleString() }}</p>
						</div>
						<div>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.errors') }}</p>
							<p class="tabular-nums text-foreground">{{ detail.usage.error_requests.toLocaleString() }}</p>
						</div>
						<div>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.tokens') }}</p>
							<p class="tabular-nums text-foreground">{{ detail.usage.input_tokens.toLocaleString() }} / {{ detail.usage.output_tokens.toLocaleString() }}</p>
						</div>
						<div>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.admin.detail.cost') }}</p>
							<p class="tabular-nums text-foreground">{{ formatMoney(detail.usage.cost_total) }}</p>
						</div>
					</div>
				</section>

				<section class="space-y-2">
					<h3 class="font-medium text-foreground">{{ store.getTranslation('developer.admin.detail.recent') }}</h3>
					<p v-if="!requests.length" class="text-muted-foreground">{{ store.getTranslation('developer.admin.detail.no_requests') }}</p>
					<div v-for="log in requests" :key="log.id" class="flex items-center justify-between gap-3 border-b border-border/60 py-1.5 last:border-b-0">
						<span class="truncate font-mono text-xs text-foreground">{{ log.public_model_id || '–' }}</span>
						<span class="shrink-0 text-xs" :class="log.status === 'error' ? 'text-destructive' : 'text-muted-foreground'">
							{{ log.status_code ?? store.getTranslation(`developer.logs.status_${log.status}`) }}
						</span>
						<span class="shrink-0 text-xs text-muted-foreground">{{ formatDate(log.created_at) }}</span>
					</div>
				</section>
			</div>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {errorMessage, formatMoney} from '~/utils/developer';
import type {DeveloperKeyRule} from '~/types/developer';
import type {AdminKeyDetail, AdminRequestLog} from '~/types/gatewayAdmin';

const props = defineProps<{open: boolean; keyId: string | null}>();
const emit = defineEmits<{'update:open': [value: boolean]}>();

const store = useMainStore();
const admin = useGatewayAdminStore();
const detail = ref<AdminKeyDetail | null>(null);
const requests = ref<AdminRequestLog[]>([]);
const loading = ref(false);

function formatDate(value: string | null): string {
	return value ? new Date(value).toLocaleString(store.base?.language ?? 'en') : '–';
}

function concurrency(value: number | null): string {
	return value === null ? store.getTranslation('developer.admin.detail.no_cap') : store.getTranslation('developer.admin.detail.cap', {count: value});
}

function ruleText(rule: DeveloperKeyRule): string {
	return store.getTranslation('developer.admin.detail.rule', {
		capacity: rule.capacity,
		metric: store.getTranslation(`developer.limits.metric_${rule.metric}`),
		period: rule.refill_period_seconds,
		route: store.getTranslation(`developer.limits.class_${rule.route_class}`),
	});
}

watch(
	() => props.open,
	async value => {
		if (!value || !props.keyId) return;
		loading.value = true;
		detail.value = null;
		requests.value = [];
		try {
			const [loaded, logs] = await Promise.all([admin.getKey(props.keyId), admin.keyRequests(props.keyId)]);
			detail.value = loaded;
			requests.value = logs.items;
		} catch (e: any) {
			store.toast(store.getTranslation('developer.admin.detail.load_error'), {type: 'error', description: errorMessage(e, '')});
			emit('update:open', false);
		} finally {
			loading.value = false;
		}
	}
);
</script>
