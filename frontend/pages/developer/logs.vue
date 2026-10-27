<template>
	<div class="w-full space-y-4">
		<div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.logs.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.logs.description') }}</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<ShadButton variant="outline" size="sm" :disabled="exporting" @click="exportLogs('csv')">
					<Download class="mr-1.5 h-3.5 w-3.5" />
					{{ store.getTranslation('developer.logs.export_csv') }}
				</ShadButton>
				<ShadButton variant="outline" size="sm" :disabled="exporting" @click="exportLogs('json')">
					<FileJson class="mr-1.5 h-3.5 w-3.5" />
					{{ store.getTranslation('developer.logs.export_json') }}
				</ShadButton>
			</div>
		</div>

		<div class="space-y-3 rounded-lg border border-border bg-card p-4">
			<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-8">
				<div v-if="developer.managedTeams.length" class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.scope') }}</ShadLabel>
					<ShadSelect v-model="scopeFilter" @update:model-value="changeScope">
						<ShadSelectTrigger class="w-full">
							<ShadSelectValue />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="mine">{{ store.getTranslation('developer.logs.scope_mine') }}</ShadSelectItem>
							<ShadSelectItem v-for="team in developer.managedTeams" :key="team.id" :value="team.id">
								{{ store.getTranslation('developer.logs.scope_service', {team: team.name}) }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div v-if="!serviceScope" class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.team') }}</ShadLabel>
					<ShadSelect v-model="teamFilter" @update:model-value="reload">
						<ShadSelectTrigger class="w-full">
							<ShadSelectValue :placeholder="store.getTranslation('developer.logs.all_teams')" />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_teams') }}</ShadSelectItem>
							<ShadSelectItem v-for="team in developer.teams" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.api_key') }}</ShadLabel>
					<ShadSelect v-model="keyFilter" @update:model-value="reload">
						<ShadSelectTrigger class="w-full">
							<ShadSelectValue :placeholder="store.getTranslation('developer.logs.all_keys')" />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_keys') }}</ShadSelectItem>
							<ShadSelectItem v-for="apiKey in keyOptions" :key="apiKey.id" :value="apiKey.id">{{ apiKey.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.status') }}</ShadLabel>
					<ShadSelect v-model="statusFilter" @update:model-value="reload">
						<ShadSelectTrigger class="w-full">
							<ShadSelectValue :placeholder="store.getTranslation('developer.logs.all_statuses')" />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_statuses') }}</ShadSelectItem>
							<ShadSelectItem value="success">{{ store.getTranslation('developer.logs.status_success') }}</ShadSelectItem>
							<ShadSelectItem value="error">{{ store.getTranslation('developer.logs.status_error') }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.error_category') }}</ShadLabel>
					<ShadSelect v-model="categoryFilter" @update:model-value="reload">
						<ShadSelectTrigger class="w-full">
							<ShadSelectValue :placeholder="store.getTranslation('developer.logs.all_categories')" />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_categories') }}</ShadSelectItem>
							<ShadSelectItem v-for="category in REQUEST_LOG_ERROR_CATEGORIES" :key="category" :value="category">
								{{ store.getTranslation(`developer.logs.category_${category}`) }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.model') }}</ShadLabel>
					<ShadInput v-model="modelFilter" :placeholder="store.getTranslation('developer.logs.model_placeholder')" @change="reload" />
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.provider') }}</ShadLabel>
					<ShadInput v-model="providerFilter" :placeholder="store.getTranslation('developer.logs.provider_placeholder')" @change="reload" />
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.from') }}</ShadLabel>
					<ShadInput v-model="fromFilter" type="date" @change="reload" />
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.logs.to') }}</ShadLabel>
					<ShadInput v-model="toFilter" type="date" @change="reload" />
				</div>
			</div>
			<div class="flex flex-col gap-3 border-t border-border pt-3 sm:flex-row sm:items-center sm:justify-between">
				<div class="flex flex-wrap items-center gap-2">
					<ShadLabel>{{ store.getTranslation('developer.logs.retention') }}</ShadLabel>
					<ShadSelect v-model="retentionDays" @update:model-value="saveRetention">
						<ShadSelectTrigger class="w-32">
							<ShadSelectValue />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="days in retentionOptions" :key="days" :value="String(days)">
								{{ store.getTranslation('developer.logs.retention_days', {count: days}) }}
							</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
					<span class="text-xs text-muted-foreground">{{ store.getTranslation('developer.logs.retention_hint') }}</span>
				</div>
				<ShadButton variant="outline" size="sm" @click="reload">
					<RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('common.refresh') }}
				</ShadButton>
			</div>
		</div>

		<div v-if="loading" class="space-y-2">
			<ShadSkeleton v-for="n in 5" :key="n" class="h-12 w-full" />
		</div>

		<div v-else-if="error" class="flex flex-col items-center gap-3 rounded-lg border border-border bg-card py-12 text-center">
			<AlertTriangle class="h-8 w-8 text-destructive" />
			<p class="font-medium text-foreground">{{ store.getTranslation('developer.logs.load_error') }}</p>
			<ShadButton variant="outline" @click="reload">{{ store.getTranslation('common.refresh') }}</ShadButton>
		</div>

		<div v-else-if="!logs.length" class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border bg-card py-16 text-center">
			<ScrollText class="h-8 w-8 text-muted-foreground opacity-60" />
			<p class="font-medium text-foreground">{{ store.getTranslation('developer.logs.empty_title') }}</p>
			<p class="max-w-sm text-sm text-muted-foreground">{{ store.getTranslation('developer.logs.empty') }}</p>
		</div>

		<div v-else class="overflow-x-auto rounded-lg border border-border bg-card">
			<table class="w-full min-w-[1100px] text-sm">
				<thead>
					<tr class="border-b border-border text-left text-xs uppercase tracking-wider text-muted-foreground">
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.time') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.request_id') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.model') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.provider') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.api_key') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.logs.status') }}</th>
						<th class="px-4 py-3 text-right font-medium">{{ store.getTranslation('developer.logs.latency') }}</th>
						<th class="px-4 py-3 text-right font-medium">{{ store.getTranslation('developer.logs.tokens') }}</th>
						<th class="px-4 py-3 text-right font-medium">{{ store.getTranslation('developer.logs.cost') }}</th>
					</tr>
				</thead>
				<tbody>
					<tr v-for="log in logs" :key="log.id" class="border-b border-border/60 last:border-b-0">
						<td class="whitespace-nowrap px-4 py-3 text-muted-foreground">{{ formatDate(log.created_at) }}</td>
						<td class="whitespace-nowrap px-4 py-3">
							<button
								v-if="log.request_id"
								type="button"
								class="inline-flex items-center gap-1.5 font-mono text-xs text-muted-foreground hover:text-foreground"
								:title="store.getTranslation('developer.logs.copy_request_id')"
								@click="copyRequestId(log.request_id)"
							>
								{{ log.request_id.slice(0, 8) }}
								<Copy class="h-3 w-3" />
							</button>
							<span v-else class="text-muted-foreground">–</span>
						</td>
						<td class="max-w-[220px] px-4 py-3">
							<p class="truncate font-mono text-foreground" :title="log.public_model_id">{{ log.public_model_id || '–' }}</p>
							<p class="text-xs text-muted-foreground">
								{{ log.request_type }}<template v-if="log.streaming"> · {{ store.getTranslation('developer.logs.streaming') }}</template>
							</p>
						</td>
						<td class="max-w-[140px] truncate px-4 py-3 text-muted-foreground" :title="log.provider ?? ''">{{ log.provider || '–' }}</td>
						<td class="max-w-[140px] truncate px-4 py-3 text-muted-foreground" :title="keyName(log.api_key_id)">{{ keyName(log.api_key_id) }}</td>
						<td class="px-4 py-3">
							<span
								class="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium"
								:class="log.status === 'error' ? 'bg-destructive/10 text-destructive' : 'bg-primary/10 text-primary'"
							>
								<span class="h-1.5 w-1.5 rounded-full" :class="log.status === 'error' ? 'bg-destructive' : 'bg-primary'" />
								{{ log.status_code ? log.status_code : store.getTranslation(`developer.logs.status_${log.status}`) }}
							</span>
							<p v-if="log.error_category" class="mt-1 text-xs text-muted-foreground">{{ store.getTranslation(`developer.logs.category_${log.error_category}`) }}</p>
						</td>
						<td class="whitespace-nowrap px-4 py-3 text-right tabular-nums text-muted-foreground">{{ formatLatency(log.latency_ms) }}</td>
						<td class="whitespace-nowrap px-4 py-3 text-right tabular-nums text-muted-foreground">
							{{ log.input_tokens.toLocaleString() }} / {{ log.output_tokens.toLocaleString() }}
						</td>
						<td class="whitespace-nowrap px-4 py-3 text-right tabular-nums text-foreground">{{ formatMoney(log.cost_total) }}</td>
					</tr>
				</tbody>
			</table>
			<div class="flex items-center justify-between border-t border-border px-4 py-2">
				<ShadButton variant="outline" size="sm" :disabled="page <= 1" @click="previousPage">
					{{ store.getTranslation('common.previous') }}
				</ShadButton>
				<span class="text-xs text-muted-foreground">{{ page }}</span>
				<ShadButton variant="outline" size="sm" :disabled="!hasMore" @click="nextPage">
					{{ store.getTranslation('common.next') }}
				</ShadButton>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, Copy, Download, FileJson, RefreshCw, ScrollText} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import {downloadFile, errorMessage, formatLatency, formatMoney, requestLogsCsv} from '~/utils/developer';
import {REQUEST_LOG_ERROR_CATEGORIES} from '~/types/developer';
import type {DeveloperRequestLog, RequestLogErrorCategory, RequestLogFilters, RequestLogStatus} from '~/types/developer';

const store = useMainStore();
const developer = useDeveloperStore();

const scopeFilter = ref('mine');
const teamFilter = ref('all');
const keyFilter = ref('all');
const statusFilter = ref('all');
const categoryFilter = ref('all');
const modelFilter = ref('');
const providerFilter = ref('');
const fromFilter = ref('');
const toFilter = ref('');
const logs = ref<DeveloperRequestLog[]>([]);
const hasMore = ref(false);
const page = ref(1);
const loading = ref(false);
const error = ref(false);
const exporting = ref(false);
const retentionDays = ref('30');
const retentionOptions = [7, 14, 30, 90, 365];

const serviceScope = computed(() => scopeFilter.value !== 'mine');

const keyOptions = computed(() =>
	serviceScope.value ? developer.teamKeys.filter(key => key.team_id === scopeFilter.value && key.owner_username === null) : developer.keys,
);

function changeScope() {
	keyFilter.value = 'all';
	teamFilter.value = 'all';
	reload();
}

function filters(): RequestLogFilters {
	return {
		scope: serviceScope.value ? 'team_service' : null,
		team_id: serviceScope.value ? scopeFilter.value : teamFilter.value === 'all' ? null : teamFilter.value,
		api_key_id: keyFilter.value === 'all' ? null : keyFilter.value,
		status: statusFilter.value === 'all' ? null : (statusFilter.value as RequestLogStatus),
		error_category: categoryFilter.value === 'all' ? null : (categoryFilter.value as RequestLogErrorCategory),
		model: modelFilter.value.trim() || null,
		provider: providerFilter.value.trim() || null,
		from: fromFilter.value || null,
		to: toFilter.value || null,
	};
}

function keyName(id: string | null): string {
	return (id && (developer.keys.find(key => key.id === id) ?? developer.teamKeys.find(key => key.id === id))?.name) || '–';
}

async function copyRequestId(id: string) {
	try {
		await navigator.clipboard.writeText(id);
		store.toast(store.getTranslation('developer.logs.request_id_copied'), {type: 'success'});
	} catch {
		store.toast(store.getTranslation('developer.logs.request_id_copy_error'), {type: 'error'});
	}
}

function formatDate(value: string) {
	return value ? new Date(value).toLocaleString(store.base?.language ?? 'en') : '';
}

async function load() {
	loading.value = true;
	error.value = false;
	try {
		const result = await developer.loadRequestLogs(filters(), page.value);
		logs.value = result.items;
		hasMore.value = result.has_more;
	} catch (e: any) {
		error.value = true;
		store.toast(store.getTranslation('developer.logs.load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
}

async function reload() {
	page.value = 1;
	await load();
}

function previousPage() {
	page.value = Math.max(1, page.value - 1);
	load();
}

function nextPage() {
	page.value += 1;
	load();
}

async function saveRetention() {
	const days = Number(retentionDays.value);
	if (!days) return;
	try {
		const saved = await developer.setRequestLogRetention(days);
		retentionDays.value = String(saved);
		store.toast(store.getTranslation('developer.logs.retention_saved'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('developer.logs.retention_error'), {type: 'error', description: errorMessage(e, '')});
	}
}

async function exportLogs(format: 'csv' | 'json') {
	exporting.value = true;
	try {
		const rows = await developer.exportRequestLogs(filters());
		const stamp = new Date().toISOString().slice(0, 10);
		if (format === 'csv') {
			downloadFile(`request-logs-${stamp}.csv`, requestLogsCsv(rows), 'text/csv');
		} else {
			downloadFile(`request-logs-${stamp}.json`, JSON.stringify(rows, null, 2), 'application/json');
		}
	} catch (e: any) {
		store.toast(store.getTranslation('developer.logs.export_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		exporting.value = false;
	}
}

onMounted(async () => {
	await Promise.all([
		developer.load().catch(() => {}),
		developer
			.getRequestLogRetention()
			.then(days => {
				retentionDays.value = String(days);
			})
			.catch(() => {}),
	]);
	await load();
});
</script>
