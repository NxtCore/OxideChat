<template>
	<div class="space-y-4">
		<div class="grid grid-cols-1 gap-3 rounded-lg border border-border bg-card p-4 sm:grid-cols-3">
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.events.days') }}</ShadLabel>
				<ShadInput v-model.number="days" type="number" min="1" max="365" @change="load" />
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.events.threshold') }}</ShadLabel>
				<ShadInput v-model.number="threshold" type="number" min="1" max="100" @change="load" />
			</div>
			<div class="flex items-end justify-end">
				<ShadButton variant="outline" size="sm" @click="load"><RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('common.refresh') }}</ShadButton>
			</div>
		</div>

		<div v-if="loading" class="space-y-2"><ShadSkeleton v-for="n in 3" :key="n" class="h-12 w-full" /></div>
		<p v-else-if="!events.length" class="rounded-lg border border-dashed border-border bg-card py-12 text-center text-sm text-muted-foreground">
			{{ store.getTranslation('developer.admin.events.empty') }}
		</p>
		<ul v-else class="divide-y divide-border rounded-lg border border-border bg-card">
			<li v-for="event in events" :key="`${event.kind}-${event.subject_id}-${event.percent_used}`" class="flex flex-col gap-1 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
				<div class="flex items-center gap-3">
					<component :is="event.kind === 'key_expiring' ? Clock : Gauge" class="h-4 w-4 shrink-0 text-muted-foreground" />
					<div>
						<p class="text-foreground">{{ event.subject_label }}</p>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation(`developer.admin.events.kind_${event.kind}`) }} · {{ store.getTranslation(`developer.admin.events.subject_${event.subject_type}`) }}
						</p>
					</div>
				</div>
				<p class="text-sm text-muted-foreground">{{ detail(event) }}</p>
			</li>
		</ul>
	</div>
</template>

<script setup lang="ts">
import {Clock, Gauge, RefreshCw} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {formatMoney} from '~/utils/format';
import type {GatewayEvent} from '~/types/gatewayAdmin';

const store = useMainStore();
const admin = useGatewayAdminStore();

const events = ref<GatewayEvent[]>([]);
const {loading, run} = useAsyncAction();
const days = ref(7);
const threshold = ref(80);

function detail(event: GatewayEvent): string {
	if (event.kind === 'key_expiring') {
		return store.getTranslation('developer.admin.events.expires_at', {date: new Date(event.expires_at ?? '').toLocaleString(store.base?.language ?? 'en')});
	}
	return store.getTranslation('developer.admin.events.budget_usage', {
		percent: event.percent_used ?? '0',
		spent: formatMoney(event.spent ?? 0),
		amount: formatMoney(event.amount ?? 0),
	});
}

async function load() {
	await run(async () => {
		events.value = await admin.listEvents(days.value, threshold.value);
	}, null, 'developer.admin.events.load_error');
}

onMounted(load);
</script>
