<template>
	<div class="space-y-4">
		<div class="grid grid-cols-1 gap-3 rounded-lg border border-border bg-card p-4 sm:grid-cols-4">
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.audit.category') }}</ShadLabel>
				<ShadSelect v-model="categoryFilter" @update:model-value="reload">
					<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem value="all">{{ store.getTranslation('developer.admin.audit.all_categories') }}</ShadSelectItem>
						<ShadSelectItem v-for="category in AUDIT_CATEGORIES" :key="category" :value="category">
							{{ store.getTranslation(`developer.admin.audit.category_${category}`) }}
						</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.logs.from') }}</ShadLabel>
				<ShadInput v-model="fromFilter" type="date" @change="reload" />
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.logs.to') }}</ShadLabel>
				<ShadInput v-model="toFilter" type="date" @change="reload" />
			</div>
			<div class="flex items-end justify-end">
				<ShadButton variant="outline" size="sm" @click="reload"><RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('common.refresh') }}</ShadButton>
			</div>
		</div>

		<div v-if="loading" class="space-y-2"><ShadSkeleton v-for="n in 5" :key="n" class="h-14 w-full" /></div>
		<p v-else-if="!entries.length" class="rounded-lg border border-dashed border-border bg-card py-12 text-center text-sm text-muted-foreground">
			{{ store.getTranslation('developer.admin.audit.empty') }}
		</p>
		<div v-else class="overflow-x-auto rounded-lg border border-border bg-card">
			<table class="w-full min-w-[900px] text-sm">
				<thead>
					<tr class="border-b border-border text-left text-xs uppercase tracking-wider text-muted-foreground">
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.time') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.audit.actor') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.audit.action') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.audit.target') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.reason') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.audit.changes') }}</th>
					</tr>
				</thead>
				<tbody>
					<tr v-for="entry in entries" :key="entry.id" class="border-b border-border/60 align-top last:border-b-0">
						<td class="whitespace-nowrap px-4 py-3 text-muted-foreground">{{ formatDateTime(entry.created_at) }}</td>
						<td class="px-4 py-3 text-foreground">{{ entry.actor_username ?? '–' }}</td>
						<td class="px-4 py-3">
							<ShadBadge variant="secondary">{{ store.getTranslation(`developer.admin.audit.category_${entry.category}`) }}</ShadBadge>
							<p class="mt-1 font-mono text-xs text-muted-foreground">{{ entry.event }}</p>
						</td>
						<td class="px-4 py-3 text-muted-foreground">{{ entry.target_label ?? entry.target_id ?? '–' }}</td>
						<td class="max-w-[200px] px-4 py-3 text-muted-foreground">{{ entry.reason ?? '–' }}</td>
						<td class="px-4 py-3">
							<pre v-if="entry.before || entry.after" class="max-w-[320px] overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs text-muted-foreground">{{ changes(entry) }}</pre>
							<span v-else class="text-muted-foreground">–</span>
						</td>
					</tr>
				</tbody>
			</table>
			<div class="flex items-center justify-between border-t border-border px-4 py-2">
				<ShadButton variant="outline" size="sm" :disabled="page <= 1" @click="goto(page - 1)">{{ store.getTranslation('common.previous') }}</ShadButton>
				<span class="text-xs text-muted-foreground">{{ page }}</span>
				<ShadButton variant="outline" size="sm" :disabled="!hasMore" @click="goto(page + 1)">{{ store.getTranslation('common.next') }}</ShadButton>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {RefreshCw} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {AUDIT_CATEGORIES} from '~/types/gatewayAdmin';
import type {AuditCategory, AuditEntry} from '~/types/gatewayAdmin';

const store = useMainStore();
const admin = useGatewayAdminStore();

const entries = ref<AuditEntry[]>([]);
const hasMore = ref(false);
const page = ref(1);
const {loading, run} = useAsyncAction();
const categoryFilter = ref('all');
const fromFilter = ref('');
const toFilter = ref('');

function changes(entry: AuditEntry): string {
	return JSON.stringify({before: entry.before, after: entry.after}, null, 1);
}

async function load() {
	await run(async () => {
		const result = await admin.listAudit(
			{
				category: categoryFilter.value === 'all' ? null : (categoryFilter.value as AuditCategory),
				from: fromFilter.value ? new Date(`${fromFilter.value}T00:00:00`).toISOString() : null,
				to: toFilter.value ? new Date(`${toFilter.value}T23:59:59`).toISOString() : null,
			},
			page.value
		);
		entries.value = result.items;
		hasMore.value = result.has_more;
	}, null, 'developer.admin.audit.load_error');
}

async function reload() {
	page.value = 1;
	await load();
}

function goto(next: number) {
	page.value = Math.max(1, next);
	load();
}

onMounted(load);
</script>
