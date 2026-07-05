<template>
	<div class="w-full">
		<!-- Header + date controls -->
		<div class="mb-6 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ title }}</h2>
				<p class="text-sm text-muted-foreground">{{ description }}</p>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<ShadButton
					v-for="preset in presets"
					:key="preset.label"
					:variant="activePreset === preset.label ? 'default' : 'outline'"
					size="sm"
					@click="applyPreset(preset)"
				>
					{{ store.getTranslation(preset.i18n) }}
				</ShadButton>
				<div class="flex items-center gap-1">
					<ShadInput v-model="from" type="date" class="h-8 w-36 text-xs" @change="load" />
					<span class="text-muted-foreground">–</span>
					<ShadInput v-model="to" type="date" class="h-8 w-36 text-xs" @change="load" />
				</div>
			</div>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-20 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<template v-else>
			<!-- Tab navigation -->
			<div class="mb-6 flex items-center gap-1 border-b border-border">
				<button
					v-for="tab in tabs"
					:key="tab.id"
					class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
					:class="activeTab === tab.id
						? 'text-foreground border-primary'
						: 'text-muted-foreground border-transparent hover:text-foreground hover:border-border'"
					@click="activeTab = tab.id"
				>
					{{ store.getTranslation(tab.i18n) }}
				</button>
			</div>

			<!-- Overview Tab -->
			<div v-if="activeTab === 'overview'">
				<!-- KPI summary row -->
				<div class="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
					<div class="rounded-lg border border-border bg-card p-4">
						<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.total_cost') }}</p>
						<p class="mt-1 text-2xl font-bold text-foreground">{{ formatMoney(totalCost) }}</p>
						<p v-if="costChange !== null" class="mt-1 text-xs" :class="costChange <= 0 ? 'text-emerald-500' : 'text-red-400'">
							<TrendingUp v-if="costChange >= 0" class="inline h-3 w-3 mr-0.5" />
							<TrendingDown v-else class="inline h-3 w-3 mr-0.5" />
							{{ costChange >= 0 ? '+' : '' }}{{ costChange.toFixed(1) }}%
						</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.requests') }}</p>
						<p class="mt-1 text-2xl font-bold text-foreground">{{ formatNumber(totalRequests) }}</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.total_tokens') }}</p>
						<p class="mt-1 text-2xl font-bold text-foreground">{{ formatTokens(totalTokens) }}</p>
					</div>
					<div class="rounded-lg border border-border bg-card p-4">
						<p class="text-xs text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.avg_cost') }}</p>
						<p class="mt-1 text-2xl font-bold text-foreground">{{ formatMoney(avgCostPerDay) }}</p>
						<p class="mt-1 text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.per_day') }}</p>
					</div>
				</div>

				<!-- Usage by model - stacked bar chart -->
				<div v-if="stackedBarData.length" class="mb-6 rounded-lg border border-border bg-card p-4">
					<div class="flex items-center justify-between mb-4">
						<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.usage_by_model') }}</h3>
					</div>
					<SettingsStackedAnalyticsChart
						:data="stackedBarData"
						:categories="stackedBarCategories"
						:format-value="formatMoney"
					/>
				</div>

				<div class="mb-6 grid gap-4 xl:grid-cols-2">
					<!-- Usage type area chart (input vs output vs reasoning) -->
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

					<!-- Request volume by model -->
					<div class="rounded-lg border border-border bg-card p-4">
						<h3 class="mb-4 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.request_volume') }}</h3>
						<div v-if="byModel.length">
							<BarChart
									:data="requestVolumeData"
									:categories="requestVolumeCategories"
									:height="200"
									:y-axis="requestVolumeYAxis"
									:x-formatter="(i: number) => requestVolumeData[i]?.label ?? ''"
									:y-formatter="(v: number) => formatNumber(v)"
									:hide-legend="true"
								:radius="4"
							/>
						</div>
						<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
					</div>
				</div>

				<!-- Top users + teams tables (admin only) -->
				<div v-if="isAdmin" class="mb-6 grid gap-4 xl:grid-cols-2">
					<!-- By user -->
					<div class="rounded-lg border border-border bg-card">
						<div class="border-b border-border px-4 py-3">
							<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_users') }}</h3>
						</div>
						<div v-if="byUser.length">
							<div
								v-for="(row, i) in byUser.slice(0, 10)"
								:key="row.id ?? row.label"
								class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2.5 text-sm last:border-0"
							>
								<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
								<span class="truncate font-medium">{{ row.label || '–' }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.request_count) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ formatTokens(tokenTotal(row)) }}</span>
								<span class="text-right font-medium text-foreground">{{ formatMoney(Number(row.cost_total)) }}</span>
							</div>
						</div>
						<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
					</div>

					<!-- By team -->
					<div class="rounded-lg border border-border bg-card">
						<div class="border-b border-border px-4 py-3">
							<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_teams') }}</h3>
						</div>
						<div v-if="byTeam.length">
							<div
								v-for="(row, i) in byTeam.slice(0, 10)"
								:key="row.id ?? row.label"
								class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem] items-center gap-2 border-b border-border/50 px-4 py-2.5 text-sm last:border-0"
							>
								<span class="text-xs text-muted-foreground">{{ i + 1 }}</span>
								<span class="truncate font-medium">{{ row.label || '–' }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ formatNumber(row.request_count) }}</span>
								<span class="text-right text-xs text-muted-foreground">{{ formatTokens(tokenTotal(row)) }}</span>
								<span class="text-right font-medium text-foreground">{{ formatMoney(Number(row.cost_total)) }}</span>
							</div>
						</div>
						<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
					</div>
				</div>
			</div>

			<!-- Trends Tab -->
			<div v-if="activeTab === 'trends'">
				<!-- Models: Spend over time + Trending -->
				<div class="mb-6">
					<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_models') }}</h3>
					<div class="grid gap-4 xl:grid-cols-[1fr_18rem]">
						<div class="rounded-lg border border-border bg-card p-4">
							<h4 class="mb-3 text-xs font-medium text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
							<SettingsStackedAnalyticsChart
								v-if="stackedBarData.length"
								:data="stackedBarData"
								:categories="stackedBarCategories"
								:format-value="formatMoney"
							/>
							<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
						</div>

						<!-- Trending sidebar -->
						<div class="rounded-lg border border-border bg-card p-4">
							<h4 class="mb-3 text-xs font-medium text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.trending') }}</h4>
							<div class="space-y-3">
								<div
									v-for="model in trendingModels"
									:key="model.label"
									class="flex items-center justify-between gap-2"
								>
									<div class="flex items-center gap-2 min-w-0">
										<span class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: model.color}" />
										<span class="text-sm font-medium truncate">{{ model.label }}</span>
									</div>
									<span class="text-xs font-medium shrink-0" :class="model.pct <= 0 ? 'text-emerald-500' : 'text-red-400'">
										{{ model.pct >= 0 ? '↑' : '↓' }} {{ Math.abs(model.pct).toFixed(0) }}%
									</span>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Top users spend over time (admin only) -->
				<div v-if="isAdmin && byUser.length" class="mb-6">
					<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_users') }}</h3>
					<div class="grid gap-4 xl:grid-cols-[1fr_18rem]">
						<div class="rounded-lg border border-border bg-card p-4">
							<h4 class="mb-3 text-xs font-medium text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
							<SettingsHorizontalAnalyticsChart
								v-if="userBarData.length"
								:rows="userBarData"
								:value-label="store.getTranslation('settings.analytics.cost')"
								color="var(--primary)"
								:format-value="formatMoney"
							/>
						</div>
						<div class="rounded-lg border border-border bg-card p-4">
							<h4 class="mb-3 text-xs font-medium text-muted-foreground uppercase tracking-wider">{{ store.getTranslation('settings.analytics.trending') }}</h4>
							<div class="space-y-3">
								<div
									v-for="(row, i) in byUser.slice(0, 8)"
									:key="row.id ?? row.label"
									class="flex items-center justify-between gap-2"
								>
									<div class="flex items-center gap-2 min-w-0">
										<span class="text-xs text-muted-foreground w-4 text-center">{{ i + 1 }}</span>
										<span class="text-sm font-medium truncate">{{ row.label }}</span>
									</div>
									<span class="text-xs font-medium text-foreground shrink-0">{{ formatMoney(Number(row.cost_total)) }}</span>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>

			<!-- Explore Tab -->
			<div v-if="activeTab === 'explore'">
				<!-- Controls -->
				<div class="mb-4 flex flex-wrap items-center gap-2">
					<div class="flex items-center gap-1.5 rounded-md border border-border bg-card px-3 py-1.5">
						<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.metric') }}</span>
						<select v-model="exploreMetric" class="bg-transparent text-sm font-medium text-foreground outline-none cursor-pointer">
							<option value="cost">{{ store.getTranslation('settings.analytics.total_cost') }}</option>
							<option value="requests">{{ store.getTranslation('settings.analytics.requests') }}</option>
							<option value="tokens_total">{{ store.getTranslation('settings.analytics.total_tokens') }}</option>
							<option value="tokens_input">{{ store.getTranslation('settings.analytics.tokens_input') }}</option>
							<option value="tokens_output">{{ store.getTranslation('settings.analytics.tokens_output') }}</option>
							<option value="tokens_reasoning">{{ store.getTranslation('settings.analytics.tokens_reasoning') }}</option>
						</select>
					</div>
					<div class="flex items-center gap-1.5 rounded-md border border-border bg-card px-3 py-1.5">
						<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.group_by') }}</span>
						<select v-model="exploreGroup" class="bg-transparent text-sm font-medium text-foreground outline-none cursor-pointer">
							<option value="model">{{ store.getTranslation('settings.analytics.model') }}</option>
							<option value="none">{{ store.getTranslation('settings.analytics.none') }}</option>
						</select>
					</div>
					<div class="flex items-center gap-1.5 rounded-md border border-border bg-card px-3 py-1.5">
						<span class="text-xs text-muted-foreground">Top</span>
						<select v-model.number="exploreTopN" class="bg-transparent text-sm font-medium text-foreground outline-none cursor-pointer">
							<option :value="5">5</option>
							<option :value="10">10</option>
							<option :value="20">20</option>
						</select>
					</div>
				</div>

				<!-- Explore chart -->
				<div class="mb-6 rounded-lg border border-border bg-card p-4">
					<SettingsStackedAnalyticsChart
						v-if="exploreChartData.length"
						:data="exploreChartData"
						:categories="exploreChartCategories"
						:format-value="exploreFormatValue"
					/>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>

				<!-- Explore detail table -->
				<div v-if="exploreTableData.length" class="rounded-lg border border-border bg-card">
					<div class="grid grid-cols-[1fr_5rem_5rem_5rem_5rem_5rem_4rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
						<span>{{ store.getTranslation('settings.analytics.label') }}</span>
						<span class="text-right">Min</span>
						<span class="text-right">Max</span>
						<span class="text-right">Avg</span>
						<span class="text-right">Sum</span>
						<span class="text-right">{{ store.getTranslation('settings.analytics.value') }}</span>
						<span class="text-right">%</span>
					</div>
					<div
						v-for="row in exploreTableData"
						:key="row.label"
						class="grid grid-cols-[1fr_5rem_5rem_5rem_5rem_5rem_4rem] items-center gap-1 border-b border-border/40 px-4 py-2.5 text-sm last:border-0"
					>
						<div class="flex items-center gap-2 min-w-0">
							<span class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: row.color}" />
							<span class="truncate font-medium">{{ row.label }}</span>
						</div>
						<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.min) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.max) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.avg) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ exploreFormatValue(row.sum) }}</span>
						<span class="text-right font-medium text-foreground">{{ exploreFormatValue(row.value) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ row.pct.toFixed(1) }}%</span>
					</div>
				</div>
			</div>

			<!-- Full model detail table (always visible below active tab) -->
			<div v-if="activeTab !== 'explore' && byModel.length" class="mt-4 rounded-lg border border-border bg-card">
				<div class="border-b border-border px-4 py-3">
					<h3 class="text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.top_models') }}</h3>
				</div>
				<div class="grid grid-cols-[1.5rem_1fr_5rem_5rem_5rem_5rem_6rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
					<span>#</span>
					<span>{{ store.getTranslation('settings.analytics.label') }}</span>
					<span class="text-right">{{ store.getTranslation('settings.analytics.requests') }}</span>
					<span class="text-right">Input</span>
					<span class="text-right">Output</span>
					<span class="text-right">Reasoning</span>
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
					<span class="text-right font-medium text-foreground">{{ formatMoney(Number(row.cost_total)) }}</span>
				</div>
			</div>
		</template>
	</div>
</template>

<script setup lang="ts">
import {Loader2, TrendingUp, TrendingDown} from 'lucide-vue-next';
import {AreaChart, BarChart} from 'vue-chrts';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';
import type {AnalyticsDayModelRow, AnalyticsRow} from '~/types/budgets';

const props = defineProps<{
	isAdmin: boolean;
}>();

const store = useMainStore();
const budgetStore = useBudgetStore();
const loading = ref(false);
const from = ref('');
const to = ref('');
const activePreset = ref('Last 30 days');
const activeTab = ref('overview');
const exploreMetric = ref('cost');
const exploreGroup = ref('model');
const exploreTopN = ref(10);

const CHART_COLORS = [
	'#ef4444', '#f97316', '#eab308', '#22c55e', '#06b6d4',
	'#3b82f6', '#8b5cf6', '#ec4899', '#f43f5e', '#14b8a6',
	'#a855f7', '#6366f1', '#d946ef', '#0ea5e9', '#84cc16',
];

const title = computed(() => props.isAdmin
	? store.getTranslation('settings.tabs.analytics')
	: store.getTranslation('settings.analytics.my_usage'));

const description = computed(() => store.getTranslation('settings.analytics.description'));

const tabs = [
	{id: 'overview', i18n: 'settings.analytics.tab_overview'},
	{id: 'trends', i18n: 'settings.analytics.tab_trends'},
	{id: 'explore', i18n: 'settings.analytics.tab_explore'},
];

const presets = [
	{label: 'Last 7 days', i18n: 'settings.analytics.last_7d', days: 7},
	{label: 'Last 30 days', i18n: 'settings.analytics.last_30d', days: 30},
	{label: 'Last 90 days', i18n: 'settings.analytics.last_90d', days: 90},
];

function isoDate(d: Date) {
	return d.toISOString().slice(0, 10);
}

function applyPreset(preset: {label: string; days: number}) {
	activePreset.value = preset.label;
	const end = new Date();
	const start = new Date();
	start.setDate(start.getDate() - preset.days);
	from.value = isoDate(start);
	to.value = isoDate(end);
	load();
}

const byModel = computed(() => props.isAdmin ? budgetStore.analytics.byModel : budgetStore.myAnalytics.byModel);
const byDay = computed(() => props.isAdmin ? budgetStore.analytics.byDay : budgetStore.myAnalytics.byDay);
const byDayModel = computed(() => props.isAdmin ? budgetStore.analytics.byDayModel : budgetStore.myAnalytics.byDayModel);
const byUser = computed(() => props.isAdmin ? budgetStore.analytics.byUser : []);
const byTeam = computed(() => props.isAdmin ? budgetStore.analytics.byTeam : []);

function tokenTotal(row: AnalyticsRow) {
	return row.input_tokens + row.output_tokens + row.reasoning_tokens;
}

const totalCost = computed(() => byModel.value.reduce((s, r) => s + Number(r.cost_total), 0));
const totalTokens = computed(() => byModel.value.reduce((s, r) => s + tokenTotal(r), 0));
const totalRequests = computed(() => byModel.value.reduce((s, r) => s + r.request_count, 0));

const dayCount = computed(() => {
	if (!from.value || !to.value) return 30;
	const d = (new Date(to.value).getTime() - new Date(from.value).getTime()) / 86_400_000;
	return Math.max(1, Math.round(d));
});

const avgCostPerDay = computed(() => (dayCount.value > 0 ? totalCost.value / dayCount.value : 0));

const costChange = computed(() => {
	const days = byDay.value;
	if (days.length < 2) return null;
	const mid = Math.floor(days.length / 2);
	const firstHalf = days.slice(0, mid).reduce((s, r) => s + Number(r.cost_total), 0);
	const secondHalf = days.slice(mid).reduce((s, r) => s + Number(r.cost_total), 0);
	if (firstHalf === 0) return null;
	return ((secondHalf - firstHalf) / firstHalf) * 100;
});

const topModelNames = computed(() => {
	const top = byModel.value.slice(0, 8).map(r => r.label);
	return top;
});

const modelColorMap = computed(() => {
	const map: Record<string, string> = {};
	topModelNames.value.forEach((name, i) => {
		map[name] = CHART_COLORS[i % CHART_COLORS.length];
	});
	map['Other'] = '#6b7280';
	return map;
});

const stackedBarData = computed(() => {
	const rows = byDayModel.value;
	if (!rows.length) return [];
	const topNames = new Set(topModelNames.value);
	const dayMap = new Map<string, Record<string, number>>();
	for (const row of rows) {
		const bucket = topNames.has(row.model_name) ? row.model_name : 'Other';
		if (!dayMap.has(row.day)) dayMap.set(row.day, {day: 0} as any);
		const entry = dayMap.get(row.day)!;
		(entry as any).day = row.day;
		entry[bucket] = (entry[bucket] || 0) + Number(row.cost_total);
	}
	return Array.from(dayMap.values());
});

const stackedBarCategories = computed(() => {
	const cats: Record<string, {name: string; color: string}> = {};
	for (const name of topModelNames.value) {
		cats[name] = {name, color: modelColorMap.value[name]};
	}
	if (byDayModel.value.some(r => !topModelNames.value.includes(r.model_name))) {
		cats['Other'] = {name: 'Other', color: '#6b7280'};
	}
	return cats;
});

const stackedBarYAxis = computed(() => Object.keys(stackedBarCategories.value));

const tokenAreaData = computed(() =>
	byDay.value.map(r => ({
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

const requestVolumeData = computed(() =>
	byModel.value.slice(0, 10).map(r => ({label: r.label, requests: r.request_count})),
);

const requestVolumeCategories = {
	requests: {name: 'Requests', color: 'var(--primary)'},
};

const requestVolumeYAxis = ['requests'];

const userBarData = computed(() =>
	byUser.value.slice(0, 8).map(row => ({label: row.label || '-', value: Number(row.cost_total)})),
);

const trendingModels = computed(() => {
	const days = [...new Set(byDayModel.value.map(row => row.day))].sort();
	const mid = Math.floor(days.length / 2);
	const firstDays = new Set(days.slice(0, mid));
	const secondDays = new Set(days.slice(mid));
	const spendByModel = new Map<string, {first: number; second: number}>();
	for (const row of byDayModel.value) {
		const spend = spendByModel.get(row.model_name) ?? {first: 0, second: 0};
		if (firstDays.has(row.day)) {
			spend.first += Number(row.cost_total);
		} else if (secondDays.has(row.day)) {
			spend.second += Number(row.cost_total);
		}
		spendByModel.set(row.model_name, spend);
	}
	return byModel.value.slice(0, 8).map((model, i) => ({
		label: model.label,
		color: CHART_COLORS[i % CHART_COLORS.length],
		pct: trendPercent(spendByModel.get(model.label)),
		cost: Number(model.cost_total),
	}));
});

function trendPercent(spend: {first: number; second: number} | undefined): number {
	if (!spend || spend.first === 0) return 0;
	return ((spend.second - spend.first) / spend.first) * 100;
}

function getMetricValue(row: AnalyticsDayModelRow): number {
	switch (exploreMetric.value) {
		case 'cost': return Number(row.cost_total);
		case 'requests': return row.request_count;
		case 'tokens_total': return row.input_tokens + row.output_tokens + row.reasoning_tokens;
		case 'tokens_input': return row.input_tokens;
		case 'tokens_output': return row.output_tokens;
		case 'tokens_reasoning': return row.reasoning_tokens;
		default: return Number(row.cost_total);
	}
}

const exploreChartData = computed(() => {
	const rows = byDayModel.value;
	if (!rows.length) return [];

	if (exploreGroup.value === 'none') {
		const dayMap = new Map<string, Record<string, number>>();
		for (const row of rows) {
			if (!dayMap.has(row.day)) dayMap.set(row.day, {} as any);
			const entry = dayMap.get(row.day)!;
			(entry as any).day = row.day;
			entry['total'] = (entry['total'] || 0) + getMetricValue(row);
		}
		return Array.from(dayMap.values());
	}

	const modelTotals = new Map<string, number>();
	for (const row of rows) {
		modelTotals.set(row.model_name, (modelTotals.get(row.model_name) || 0) + getMetricValue(row));
	}
	const topNames = [...modelTotals.entries()]
		.sort((a, b) => b[1] - a[1])
		.slice(0, exploreTopN.value)
		.map(([name]) => name);
	const topSet = new Set(topNames);

	const dayMap = new Map<string, Record<string, number>>();
	for (const row of rows) {
		const bucket = topSet.has(row.model_name) ? row.model_name : 'Other';
		if (!dayMap.has(row.day)) dayMap.set(row.day, {} as any);
		const entry = dayMap.get(row.day)!;
		(entry as any).day = row.day;
		entry[bucket] = (entry[bucket] || 0) + getMetricValue(row);
	}
	return Array.from(dayMap.values());
});

const exploreChartCategories = computed(() => {
	if (exploreGroup.value === 'none') {
		return {total: {name: 'Total', color: 'var(--primary)'}};
	}
	const modelTotals = new Map<string, number>();
	for (const row of byDayModel.value) {
		modelTotals.set(row.model_name, (modelTotals.get(row.model_name) || 0) + getMetricValue(row));
	}
	const topNames = [...modelTotals.entries()]
		.sort((a, b) => b[1] - a[1])
		.slice(0, exploreTopN.value)
		.map(([name]) => name);

	const cats: Record<string, {name: string; color: string}> = {};
	topNames.forEach((name, i) => {
		cats[name] = {name, color: CHART_COLORS[i % CHART_COLORS.length]};
	});
	const hasOther = byDayModel.value.some(r => !topNames.includes(r.model_name));
	if (hasOther) {
		cats['Other'] = {name: 'Other', color: '#6b7280'};
	}
	return cats;
});

const exploreChartYAxis = computed(() => Object.keys(exploreChartCategories.value));

const exploreYFormatter = computed(() => {
	if (exploreMetric.value === 'cost') return (v: number) => `$${Number(v).toFixed(3)}`;
	return (v: number) => formatNumber(v);
});

const exploreTableData = computed(() => {
	if (exploreGroup.value === 'none') return [];

	const modelDailyValues = new Map<string, number[]>();
	for (const row of byDayModel.value) {
		const vals = modelDailyValues.get(row.model_name) || [];
		vals.push(getMetricValue(row));
		modelDailyValues.set(row.model_name, vals);
	}

	const totalAll = [...modelDailyValues.values()].reduce((s, vals) => s + vals.reduce((a, b) => a + b, 0), 0);

	const modelTotals = [...modelDailyValues.entries()].map(([name, vals]) => ({
		label: name,
		sum: vals.reduce((a, b) => a + b, 0),
		vals,
	}));
	modelTotals.sort((a, b) => b.sum - a.sum);

	return modelTotals.slice(0, exploreTopN.value).map((m, i) => ({
		label: m.label,
		color: CHART_COLORS[i % CHART_COLORS.length],
		min: Math.min(...m.vals),
		max: Math.max(...m.vals),
		avg: m.sum / Math.max(1, m.vals.length),
		sum: m.sum,
		value: m.sum,
		pct: totalAll > 0 ? (m.sum / totalAll) * 100 : 0,
	}));
});

function exploreFormatValue(v: number): string {
	if (exploreMetric.value === 'cost') return formatMoney(v);
	return formatNumber(v);
}

function formatMoney(value: number) {
	if (value >= 1) return `$${value.toFixed(2)}`;
	return `$${value.toFixed(4)}`;
}

function formatTokens(n: number) {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return String(n);
}

function formatNumber(n: number) {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return n.toLocaleString();
}

async function load() {
	loading.value = true;
	try {
		const params = {
			from: from.value ? `${from.value}T00:00:00Z` : undefined,
			to: to.value ? `${to.value}T23:59:59Z` : undefined,
		};
		if (props.isAdmin) {
			await budgetStore.fetchAllAnalytics(params);
		} else {
			await budgetStore.fetchMyAnalytics(params);
		}
	} finally {
		loading.value = false;
	}
}

onMounted(() => {
	applyPreset(presets[1]);
});
</script>
