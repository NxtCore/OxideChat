<template>
	<div>
		<div class="mb-4 flex flex-wrap items-center gap-2">
			<ShadSelect v-for="s in selects" :key="s.key" v-model="explore[s.key]">
				<ShadSelectTrigger size="sm" class="h-8 max-w-full min-w-0 bg-card" :class="s.width">
					<ShadSelectValue>
						<span class="text-xs font-medium text-muted-foreground">{{ s.label }}</span>
						<span class="font-semibold text-foreground">{{ s.options.find(option => option.value === explore[s.key])?.label }}</span>
					</ShadSelectValue>
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem v-for="option in s.options" :key="option.value" :value="option.value">
						{{ option.label }}
					</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
		</div>

		<div class="mb-6 rounded-lg border border-border bg-card px-4 py-5">
			<SettingsStackedAnalyticsChart v-if="chartData.length" :data="chartData" :categories="chartCategories" :format-value="formatValue" />
			<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
		</div>

		<div v-if="tableData.length" class="rounded-lg border border-border bg-card">
			<div class="overflow-x-auto">
				<div class="min-w-[38rem]">
					<div class="grid grid-cols-[minmax(10rem,1fr)_4.75rem_4.75rem_4.75rem_4.75rem_5.25rem_6.25rem] border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
						<span>{{ translateOr('settings.analytics.label', 'Label') }}</span>
						<span class="text-right">{{ translateOr('settings.analytics.min', 'Min') }}</span>
						<span class="text-right">{{ translateOr('settings.analytics.max', 'Max') }}</span>
						<span class="text-right">{{ translateOr('settings.analytics.avg', 'Avg') }}</span>
						<span class="text-right">{{ translateOr('settings.analytics.sum', 'Sum') }}</span>
						<span class="text-right">{{ translateOr('settings.analytics.value', 'Value') }}</span>
						<span class="text-right">%</span>
					</div>
					<div
						v-for="row in tableData"
						:key="row.label"
						class="grid grid-cols-[minmax(10rem,1fr)_4.75rem_4.75rem_4.75rem_4.75rem_5.25rem_6.25rem] items-center gap-1 border-b border-border/40 px-4 py-2.5 text-sm last:border-0"
					>
						<div class="flex items-center gap-2 min-w-0">
							<img v-if="row.icon?.type === 'png'" :src="row.icon.icon" class="h-5 w-5 shrink-0 rounded-md bg-muted object-cover" alt="" />
							<div
								v-else-if="row.icon?.type === 'svg'"
								class="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
								v-html="row.icon.icon"
							/>
							<span v-else class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: row.color}" />
							<span class="truncate font-medium">{{ row.label }}</span>
						</div>
						<span class="text-right text-xs text-muted-foreground">{{ formatValue(row.min) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ formatValue(row.max) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ formatValue(row.avg) }}</span>
						<span class="text-right text-xs text-muted-foreground">{{ formatValue(row.sum) }}</span>
						<span class="text-right font-medium text-foreground">{{ formatValue(row.value) }}</span>
						<div class="flex items-center justify-end gap-2">
							<div class="h-2 w-12 overflow-hidden rounded-full bg-muted">
								<div class="h-full rounded-full" :style="{width: `${Math.min(100, row.pct)}%`, backgroundColor: row.color}" />
							</div>
							<span class="w-9 text-right text-xs text-muted-foreground tabular-nums">{{ row.pct.toFixed(1) }}%</span>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';
import type {AnalyticsDayModelRow, AnalyticsRow} from '~/types/budgets';

const props = defineProps<{
	byModel: AnalyticsRow[];
	byDayModel: AnalyticsDayModelRow[];
	byUser: AnalyticsRow[];
}>();

const store = useMainStore();
const explore = reactive<Record<string, any>>({metric: 'cost', group: 'model', topN: 10, rollup: 'hourly'});

const selects = computed(() => [
	{
		key: 'metric',
		label: translateOr('settings.analytics.metric', 'Metric'),
		width: 'sm:min-w-52',
		options: [
			{value: 'requests', label: translateOr('settings.analytics.request_count', 'Request count')},
			{value: 'cost', label: translateOr('settings.analytics.total_usage_dollars', 'Total usage ($)')},
			{value: 'tokens_total', label: translateOr('settings.analytics.tokens_total', 'Tokens (total)')},
			{value: 'tokens_input', label: translateOr('settings.analytics.tokens_prompt', 'Tokens (prompt)')},
			{value: 'tokens_output', label: translateOr('settings.analytics.tokens_completion', 'Tokens (completion)')},
			{value: 'tokens_reasoning', label: translateOr('settings.analytics.reasoning_tokens', 'Reasoning tokens')},
		],
	},
	{
		key: 'group',
		label: translateOr('settings.analytics.group_by', 'Group by'),
		width: 'sm:min-w-44',
		options: [
			{value: 'model', label: translateOr('settings.analytics.model', 'Model')},
			{value: 'user', label: translateOr('settings.analytics.user', 'User')},
		],
	},
	{
		key: 'topN',
		label: translateOr('settings.analytics.top', 'Top'),
		width: 'sm:min-w-24',
		options: [5, 10, 20].map(n => ({value: n, label: String(n)})),
	},
	{
		key: 'rollup',
		label: translateOr('settings.analytics.rollup', 'Rollup'),
		width: 'sm:min-w-36',
		options: [
			{value: 'hourly', label: translateOr('settings.analytics.hourly', 'Hourly')},
			{value: 'daily', label: translateOr('settings.analytics.daily', 'Daily')},
			{value: 'weekly', label: translateOr('settings.analytics.weekly', 'Weekly')},
		],
	},
]);

function metricValue(row: AnalyticsRow | AnalyticsDayModelRow): number {
	switch (explore.metric) {
		case 'requests': return row.request_count;
		case 'tokens_total': return tokenTotal(row);
		case 'tokens_input': return row.input_tokens;
		case 'tokens_output': return row.output_tokens;
		case 'tokens_reasoning': return row.reasoning_tokens;
		default: return Number(row.cost_total);
	}
}

function formatValue(v: number): string {
	return explore.metric === 'cost' ? formatCost(v) : formatNumber(v);
}

function bucketDay(day: string): string {
	if (explore.rollup !== 'weekly') return day;
	const date = new Date(`${day}T00:00:00Z`);
	if (Number.isNaN(date.getTime())) return day;
	const weekStart = new Date(date);
	const dayOfWeek = weekStart.getUTCDay() || 7;
	weekStart.setUTCDate(weekStart.getUTCDate() - dayOfWeek + 1);
	return weekStart.toISOString().slice(0, 10);
}

const topModelNames = computed(() => {
	const totals = new Map<string, number>();
	for (const row of props.byDayModel) {
		totals.set(row.model_name, (totals.get(row.model_name) || 0) + metricValue(row));
	}
	return [...totals.entries()]
		.sort((a, b) => b[1] - a[1])
		.slice(0, explore.topN)
		.map(([name]) => name);
});

const chartData = computed(() => {
	if (explore.group === 'user') {
		if (!tableData.value.length) return [];
		return [tableData.value.reduce((entry, row) => ({...entry, [row.label]: row.value}), {day: store.getTranslation('settings.analytics.total')} as Record<string, number | string>)];
	}

	const topSet = new Set(topModelNames.value);
	const dayMap = new Map<string, Record<string, number | string>>();
	for (const row of props.byDayModel) {
		const bucket = topSet.has(row.model_name) ? row.model_name : 'Other';
		const day = bucketDay(row.day);
		const entry = dayMap.get(day) ?? {day};
		entry[bucket] = ((entry[bucket] as number) || 0) + metricValue(row);
		dayMap.set(day, entry);
	}
	return Array.from(dayMap.values());
});

const chartCategories = computed(() => {
	const cats: Record<string, {name: string; color: string}> = {};
	const names = explore.group === 'user' ? tableData.value.map(row => row.label) : topModelNames.value;
	names.forEach((name, i) => {
		cats[name] = {name, color: ANALYTICS_COLORS[i % ANALYTICS_COLORS.length]};
	});
	if (explore.group === 'model' && props.byDayModel.some(r => !names.includes(r.model_name))) {
		cats['Other'] = {name: store.getTranslation('settings.analytics.other'), color: '#6b7280'};
	}
	return cats;
});

const tableData = computed(() => {
	if (explore.group !== 'model') {
		const rows = props.byUser.map(row => ({label: row.label || '-', value: metricValue(row)})).sort((a, b) => b.value - a.value);
		const total = rows.reduce((sum, row) => sum + row.value, 0);
		return rows.slice(0, explore.topN).map((row, i) => ({
			label: row.label,
			color: ANALYTICS_COLORS[i % ANALYTICS_COLORS.length],
			icon: null,
			min: row.value,
			max: row.value,
			avg: row.value,
			sum: row.value,
			value: row.value,
			pct: total > 0 ? (row.value / total) * 100 : 0,
		}));
	}

	const dailyValues = new Map<string, number[]>();
	for (const row of props.byDayModel) {
		dailyValues.set(row.model_name, [...(dailyValues.get(row.model_name) ?? []), metricValue(row)]);
	}
	const models = [...dailyValues.entries()]
		.map(([label, vals]) => ({label, vals, sum: vals.reduce((a, b) => a + b, 0)}))
		.sort((a, b) => b.sum - a.sum);
	const totalAll = models.reduce((s, m) => s + m.sum, 0);

	return models.slice(0, explore.topN).map((m, i) => ({
		label: m.label,
		color: ANALYTICS_COLORS[i % ANALYTICS_COLORS.length],
		icon: modelIcon(m.label, props.byModel.find(row => row.label === m.label)?.id ?? null),
		min: Math.min(...m.vals),
		max: Math.max(...m.vals),
		avg: m.sum / Math.max(1, m.vals.length),
		sum: m.sum,
		value: m.sum,
		pct: totalAll > 0 ? (m.sum / totalAll) * 100 : 0,
	}));
});
</script>
