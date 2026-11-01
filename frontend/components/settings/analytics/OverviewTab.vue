<template>
	<div>
		<div class="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
			<div v-for="kpi in kpis" :key="kpi.label" class="rounded-lg border border-border bg-card p-4">
				<div class="flex items-start justify-between gap-2">
					<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation(`settings.analytics.${kpi.label}`) }}</p>
					<svg viewBox="0 0 64 24" class="h-8 w-16 shrink-0" aria-hidden="true">
						<path :d="kpi.path" fill="none" stroke-width="1.5" class="stroke-primary" :class="kpi.dim && 'opacity-60'" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</div>
				<p class="mt-1 text-2xl font-bold text-foreground tabular-nums">{{ kpi.value }}</p>
				<p v-if="kpi.change != null" class="mt-1 text-xs flex items-center gap-0.5" :class="kpi.change <= 0 ? 'text-emerald-500' : 'text-red-400'">
					<TrendingUp v-if="kpi.change >= 0" class="h-3 w-3" />
					<TrendingDown v-else class="h-3 w-3" />
					{{ kpi.change >= 0 ? '+' : '' }}{{ kpi.change.toFixed(1) }}%
				</p>
				<p v-if="kpi.sub" class="mt-1 text-xs text-muted-foreground">{{ store.getTranslation(kpi.sub) }}</p>
			</div>
		</div>

		<div v-if="isAdmin" class="mb-6 grid gap-4 xl:grid-cols-2">
			<div v-for="card in topCards" :key="card.title" class="rounded-lg border border-border bg-card">
				<div class="border-b border-border px-4 py-3">
					<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation(card.title) }}</h3>
				</div>
				<div v-if="card.rows.length" class="overflow-x-auto">
					<div class="min-w-[24rem]">
						<div class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2 text-xs font-medium text-muted-foreground">
							<span>#</span>
							<span></span>
							<span class="text-right">{{ store.getTranslation('settings.analytics.requests') }}</span>
							<span class="text-right">{{ store.getTranslation('settings.analytics.total_tokens_short') }}</span>
							<span class="text-right">{{ store.getTranslation('settings.analytics.cost') }}</span>
						</div>
						<div
							v-for="(row, i) in card.rows"
							:key="row.key"
							class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2.5 text-sm last:border-0"
						>
							<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
							<div class="flex min-w-0 items-center gap-2">
								<div class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium uppercase text-muted-foreground">
									<KeyRound v-if="card.isKey" class="h-3 w-3" />
									<template v-else>{{ (row.label || '?').slice(0, 1) }}</template>
								</div>
								<span class="truncate font-medium">{{ row.label || '–' }}</span>
							</div>
							<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.requests) }}</span>
							<span class="text-right text-xs text-muted-foreground">{{ formatTokens(row.tokens) }}</span>
							<span class="text-right font-medium text-foreground">{{ formatCost(row.cost) }}</span>
						</div>
					</div>
				</div>
				<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
			</div>
		</div>

		<div v-if="stacked.data.length" class="mb-6 rounded-lg border border-border bg-card p-4">
			<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.usage_by_model') }}</h3>
			<SettingsStackedAnalyticsChart :data="stacked.data" :categories="stacked.categories" :format-value="formatCost" />
		</div>

		<div class="mb-6 grid gap-4 xl:grid-cols-2">
			<div v-if="byDay.length" class="rounded-lg border border-border bg-card p-4">
				<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.token_split') }}</h3>
				<AreaChart
					:stacked="true"
					:data="tokenAreaData"
					:categories="tokenAreaCategories"
					:height="200"
					:x-formatter="(i: number) => tokenAreaData[i]?.day ?? ''"
					:y-formatter="(v: number) => formatTokens(v)"
					:y-grid-line="true"
					:x-num-ticks="6"
				/>
			</div>

			<div class="rounded-lg border border-border bg-card p-4">
				<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.request_volume') }}</h3>
				<BarChart
					v-if="byModel.length"
					:data="requestVolumeData"
					:categories="requestVolumeCategories"
					:height="200"
					:y-axis="['requests']"
					:x-formatter="(i: number) => requestVolumeData[i]?.label ?? ''"
					:y-formatter="(v: number) => formatNumber(v)"
					:hide-legend="true"
					:radius="4"
				/>
				<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
			</div>
		</div>

		<div v-if="byModel.length" class="mt-4 rounded-lg border border-border bg-card">
			<div class="border-b border-border px-4 py-3">
				<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_models') }}</h3>
			</div>
			<div class="overflow-x-auto">
				<div class="min-w-[32rem]">
					<div class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem_5rem_6rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
						<span>#</span>
						<span>{{ store.getTranslation('settings.analytics.label') }}</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.requests') }}</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.input') }}</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.output') }}</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.reasoning') }}</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.cost') }}</span>
					</div>
					<div
						v-for="(row, i) in byModel"
						:key="row.id ?? row.label"
						class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem_5rem_6rem] items-center gap-1 border-b border-border/40 px-4 py-2.5 text-sm last:border-0"
					>
						<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
						<span class="truncate font-medium">{{ row.label || '–' }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.request_count) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ row.input_tokens.toLocaleString() }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ row.output_tokens.toLocaleString() }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ row.reasoning_tokens.toLocaleString() }}</span>
						<span class="text-right font-medium text-foreground">{{ formatCost(Number(row.cost_total)) }}</span>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {TrendingUp, TrendingDown, KeyRound} from 'lucide-vue-next';
import {AreaChart, BarChart} from 'vue-chrts';
import {useMainStore} from '@/stores';
import type {AnalyticsRow, ApiKeyAnalyticsRow} from '~/types/budgets';

const props = defineProps<{
	isAdmin: boolean;
	byModel: AnalyticsRow[];
	byDay: AnalyticsRow[];
	byUser: AnalyticsRow[];
	byApiKey: ApiKeyAnalyticsRow[];
	stacked: {data: Record<string, any>[]; categories: Record<string, {name: string; color: string}>};
	totalCost: number;
	dayCount: number;
}>();

const store = useMainStore();

const costChange = computed(() => {
	const days = props.byDay;
	if (days.length < 2) return null;
	const mid = Math.floor(days.length / 2);
	const firstHalf = days.slice(0, mid).reduce((s, r) => s + Number(r.cost_total), 0);
	const secondHalf = days.slice(mid).reduce((s, r) => s + Number(r.cost_total), 0);
	if (firstHalf === 0) return null;
	return ((secondHalf - firstHalf) / firstHalf) * 100;
});

const kpis = computed(() => {
	const costPath = sparklinePath(props.byDay.map(r => Number(r.cost_total)));
	return [
		{label: 'total_cost', path: costPath, value: formatCost(props.totalCost), change: costChange.value},
		{
			label: 'requests',
			path: sparklinePath(props.byDay.map(r => r.request_count)),
			value: formatNumber(props.byModel.reduce((s, r) => s + r.request_count, 0)),
		},
		{
			label: 'total_tokens',
			path: sparklinePath(props.byDay.map(tokenTotal)),
			value: formatTokens(props.byModel.reduce((s, r) => s + tokenTotal(r), 0)),
		},
		{label: 'avg_cost', path: costPath, value: formatCost(props.totalCost / props.dayCount), dim: true, sub: 'settings.analytics.per_day'},
	];
});

const topCards = computed(() => [
	{
		title: 'settings.analytics.top_api_keys',
		isKey: true,
		rows: props.byApiKey.slice(0, 8).map(r => ({key: r.id ?? 'unknown', label: apiKeyLabel(r), requests: r.request_count, tokens: tokenTotal(r), cost: Number(r.cost_total)})),
	},
	{
		title: 'settings.analytics.top_users',
		isKey: false,
		rows: props.byUser.slice(0, 8).map(r => ({key: r.id ?? r.label, label: r.label, requests: r.request_count, tokens: tokenTotal(r), cost: Number(r.cost_total)})),
	},
]);

const tokenAreaData = computed(() =>
	props.byDay.map(r => ({
		day: r.label,
		input: r.input_tokens,
		output: r.output_tokens,
		reasoning: r.reasoning_tokens,
	})),
);

const tokenAreaCategories = {
	input: {name: 'Input', color: '#3b82f6'},
	output: {name: 'Output', color: '#8b5cf6'},
	reasoning: {name: 'Reasoning', color: '#06b6d4'},
};

const requestVolumeData = computed(() => props.byModel.slice(0, 10).map(r => ({label: r.label, requests: r.request_count})));

const requestVolumeCategories = {
	requests: {name: 'Requests', color: 'var(--primary)'},
};
</script>
