<template>
	<div class="w-full space-y-6">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.overview.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.overview.description') }}</p>
		</div>

		<div v-if="budgetBlocked" class="flex items-start gap-3 rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-foreground" role="alert">
			<AlertTriangle class="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
			<div>
				<p class="font-medium">{{ store.getTranslation('developer.overview.budget_exhausted') }}</p>
				<p class="text-muted-foreground">{{ store.getTranslation('developer.overview.budget_exhausted_hint') }}</p>
			</div>
		</div>

		<div v-if="developer.error" class="flex items-center justify-between gap-3 rounded-lg border border-border bg-card p-4">
			<p class="text-sm text-foreground">{{ store.getTranslation('developer.errors.load') }}</p>
			<ShadButton variant="outline" size="sm" @click="load">{{ store.getTranslation('common.refresh') }}</ShadButton>
		</div>

		<div class="rounded-lg border border-border bg-card p-4">
			<p class="mb-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.overview.base_url') }}</p>
			<div class="flex items-center gap-2">
				<code class="min-w-0 flex-1 break-all rounded-md bg-muted px-3 py-2 font-mono text-sm text-foreground" data-testid="base-url">{{ baseUrl }}</code>
				<ShadButton variant="outline" size="icon" :aria-label="store.getTranslation('developer.reveal.copy')" @click="store.copyToClipboard(baseUrl)">
					<Copy class="h-4 w-4" />
				</ShadButton>
			</div>
		</div>

		<div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
			<div class="rounded-lg border border-border bg-card p-4">
				<p class="text-xs uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.overview.active_keys') }}</p>
				<ShadSkeleton v-if="developer.loading && !developer.loaded" class="mt-2 h-8 w-12" />
				<p v-else class="mt-1 text-2xl font-semibold tabular-nums text-foreground">{{ developer.activeKeys.length }}</p>
			</div>
			<div class="rounded-lg border border-border bg-card p-4">
				<p class="text-xs uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.overview.teams') }}</p>
				<ShadSkeleton v-if="developer.loading && !developer.loaded" class="mt-2 h-8 w-12" />
				<p v-else class="mt-1 text-2xl font-semibold tabular-nums text-foreground">{{ developer.teams.length }}</p>
			</div>
			<div class="rounded-lg border border-border bg-card p-4">
				<p class="text-xs uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.overview.spend') }}</p>
				<ShadSkeleton v-if="spend === null && !spendFailed" class="mt-2 h-8 w-20" />
				<p v-else class="mt-1 text-2xl font-semibold tabular-nums text-foreground">{{ spendFailed ? '–' : formatMoney(spend ?? 0) }}</p>
				<p class="mt-1 text-xs text-muted-foreground">{{ store.getTranslation('developer.overview.spend_hint') }}</p>
			</div>
			<div class="rounded-lg border border-border bg-card p-4">
				<p class="text-xs uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.overview.health') }}</p>
				<p class="mt-1 flex items-center gap-2 text-lg font-semibold text-foreground">
					<span class="h-2.5 w-2.5 rounded-full" :class="health === 'ok' ? 'bg-primary' : health === 'down' ? 'bg-destructive' : 'bg-muted-foreground'" />
					{{ store.getTranslation(`developer.overview.health_${health}`) }}
				</p>
			</div>
		</div>

		<div v-if="developer.loaded && !developer.keys.length" class="flex flex-col items-start gap-3 rounded-lg border border-dashed border-border bg-card p-6 sm:flex-row sm:items-center sm:justify-between">
			<div>
				<p class="font-medium text-foreground">{{ store.getTranslation('developer.overview.get_started') }}</p>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.overview.get_started_hint') }}</p>
			</div>
			<ShadButton as-child><NuxtLink to="/developer/keys">{{ store.getTranslation('developer.keys.create') }}</NuxtLink></ShadButton>
		</div>

		<div>
			<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('developer.overview.team_models') }}</h3>
			<DeveloperTeamModels :teams="developer.teams" :loading="developer.loading && !developer.loaded" />
		</div>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import {useBudgetStore} from '@/stores/budgetStore';
import type {AnalyticsRow} from '~/types/budgets';

const store = useMainStore();
const developer = useDeveloperStore();
const budgetStore = useBudgetStore();
const {$customFetch} = useNuxtApp();

const baseUrl = ref('');
const spend = ref<number | null>(null);
const spendFailed = ref(false);
const health = ref<'checking' | 'ok' | 'down'>('checking');

const budgetBlocked = computed(() => budgetStore.myStatus?.decision === 'block');

async function load() {
	await developer.load().catch(() => {});
}

async function loadSpend() {
	const from = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString();
	try {
		const rows = (await $customFetch<AnalyticsRow[]>('/api/v1/me/analytics', {params: {from, group_by: 'model'}})) ?? [];
		spend.value = rows.reduce((sum, row) => sum + Number(row.cost_total), 0);
	} catch {
		spendFailed.value = true;
	}
}

async function loadHealth() {
	try {
		await $customFetch('/api/v1/health');
		health.value = 'ok';
	} catch {
		health.value = 'down';
	}
}

onMounted(() => {
	baseUrl.value = gatewayBaseUrl();
	load();
	loadSpend();
	loadHealth();
	budgetStore.fetchMyBudget().catch(() => {});
});
</script>
