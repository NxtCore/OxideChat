<template>
	<div class="space-y-7">
		<section>
			<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_models') }}</h3>
			<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
						<span class="text-xs font-medium text-muted-foreground tabular-nums">{{ formatCost(totalCost) }}</span>
					</div>
					<SettingsStackedAnalyticsChart v-if="stacked.data.length" :data="stacked.data" :categories="stacked.categories" :format-value="formatCost" />
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
						<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.spend') }}</span>
					</div>
					<div v-if="trendingModels.length" class="space-y-4">
						<div v-for="model in trendingModels" :key="model.label" class="grid grid-cols-[minmax(0,1fr)_5rem_4.5rem] items-center gap-3">
							<div class="flex min-w-0 items-center gap-2">
								<img v-if="model.icon?.type === 'png'" :src="model.icon.icon" class="h-5 w-5 shrink-0 rounded-md bg-muted object-cover" alt="" />
								<div
									v-else-if="model.icon?.type === 'svg'"
									class="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground [&>svg]:h-full [&>svg]:w-full"
									v-html="model.icon.icon"
								/>
								<span v-else class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: model.color}" />
								<div class="min-w-0">
									<p class="truncate text-sm font-semibold text-foreground">{{ model.label }}</p>
									<p class="truncate text-xs text-muted-foreground">
										{{ formatNumber(model.requests) }} {{ store.getTranslation('settings.analytics.requests') }} | {{ formatTokens(model.tokens) }}
										{{ store.getTranslation('settings.analytics.tokens') }}
									</p>
								</div>
							</div>
							<svg viewBox="0 0 64 24" class="h-6 w-16 overflow-visible" aria-hidden="true">
								<path :d="model.sparkline" fill="none" stroke-width="1.5" :stroke="model.color" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
							<div class="text-right">
								<p class="text-sm font-semibold text-foreground tabular-nums">{{ formatCost(model.cost) }}</p>
								<p class="text-xs font-medium tabular-nums" :class="trendClass(model.pct)">{{ trendLabel(model.pct) }}</p>
							</div>
						</div>
					</div>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
			</div>
		</section>

		<section v-if="isAdmin">
			<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_api_keys') }}</h3>
			<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
						<span class="text-xs font-medium text-muted-foreground tabular-nums">{{ formatCost(apiKeyTotalCost) }}</span>
					</div>
					<SettingsHorizontalAnalyticsChart
						v-if="apiKeyBarData.length"
						:rows="apiKeyBarData"
						:value-label="store.getTranslation('settings.analytics.cost')"
						color="var(--primary)"
						:format-value="formatCost"
					/>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
						<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.trend') }}</span>
					</div>
					<p class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
			</div>
		</section>

		<section v-if="isAdmin">
			<h3 class="mb-3 text-sm font-semibold text-foreground">{{ store.getTranslation('settings.analytics.section_user') }}</h3>
			<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.spend_over_time') }}</h4>
						<span class="text-xs font-medium text-muted-foreground tabular-nums">{{ formatCost(userTotalCost) }}</span>
					</div>
					<SettingsHorizontalAnalyticsChart
						v-if="userBarData.length"
						:rows="userBarData"
						:value-label="store.getTranslation('settings.analytics.cost')"
						color="var(--primary)"
						:format-value="formatCost"
					/>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="mb-4 flex items-center justify-between gap-3">
						<h4 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('settings.analytics.trending') }}</h4>
						<span class="text-xs text-muted-foreground">{{ store.getTranslation('settings.analytics.spend') }}</span>
					</div>
					<div v-if="trendingUsers.length" class="space-y-4">
						<div v-for="user in trendingUsers" :key="user.id ?? user.label" class="grid grid-cols-[minmax(0,1fr)_5rem_4.5rem] items-center gap-3">
							<div class="flex min-w-0 items-center gap-2">
								<div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold uppercase text-muted-foreground">
									{{ user.initial }}
								</div>
								<div class="min-w-0">
									<p class="truncate text-sm font-semibold text-foreground">{{ user.label }}</p>
									<p class="truncate text-xs text-muted-foreground">
										{{ formatNumber(user.requests) }} {{ store.getTranslation('settings.analytics.requests') }} | {{ formatTokens(user.tokens) }}
										{{ store.getTranslation('settings.analytics.tokens') }}
									</p>
								</div>
							</div>
							<svg viewBox="0 0 64 24" class="h-6 w-16 overflow-visible" aria-hidden="true">
								<path :d="user.sparkline" fill="none" stroke-width="1.5" class="stroke-primary" stroke-linecap="round" stroke-linejoin="round" />
							</svg>
							<div class="text-right">
								<p class="text-sm font-semibold text-foreground tabular-nums">{{ formatCost(user.cost) }}</p>
								<p class="text-xs font-medium text-muted-foreground tabular-nums">{{ user.share.toFixed(0) }}%</p>
							</div>
						</div>
					</div>
					<p v-else class="py-8 text-center text-sm text-muted-foreground">{{ store.getTranslation('settings.analytics.no_data') }}</p>
				</div>
			</div>
		</section>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';
import type {AnalyticsDayModelRow, AnalyticsRow, ApiKeyAnalyticsRow} from '~/types/budgets';

const props = defineProps<{
	isAdmin: boolean;
	byModel: AnalyticsRow[];
	byDayModel: AnalyticsDayModelRow[];
	byUser: AnalyticsRow[];
	byApiKey: ApiKeyAnalyticsRow[];
	stacked: {data: Record<string, any>[]; categories: Record<string, {name: string; color: string}>};
	totalCost: number;
}>();

const store = useMainStore();

const userBarData = computed(() => props.byUser.slice(0, 8).map(row => ({label: row.label || '-', value: Number(row.cost_total)})));
const userTotalCost = computed(() => props.byUser.reduce((sum, row) => sum + Number(row.cost_total), 0));
const apiKeyBarData = computed(() => props.byApiKey.slice(0, 8).map(row => ({label: apiKeyLabel(row), value: Number(row.cost_total)})));
const apiKeyTotalCost = computed(() => props.byApiKey.reduce((sum, row) => sum + Number(row.cost_total), 0));

const trendingModels = computed(() => {
	const days = [...new Set(props.byDayModel.map(row => row.day))].sort();
	const mid = Math.floor(days.length / 2);
	const firstDays = new Set(days.slice(0, mid));
	const secondDays = new Set(days.slice(mid));
	const spendByModel = new Map<string, {first: number; second: number}>();
	const seriesByModel = new Map<string, number[]>();
	for (const row of props.byDayModel) {
		const spend = spendByModel.get(row.model_name) ?? {first: 0, second: 0};
		if (firstDays.has(row.day)) {
			spend.first += Number(row.cost_total);
		} else if (secondDays.has(row.day)) {
			spend.second += Number(row.cost_total);
		}
		spendByModel.set(row.model_name, spend);
		const series = seriesByModel.get(row.model_name) ?? Array.from({length: days.length}, () => 0);
		const dayIndex = days.indexOf(row.day);
		if (dayIndex >= 0) {
			series[dayIndex] += Number(row.cost_total);
		}
		seriesByModel.set(row.model_name, series);
	}
	return props.byModel.slice(0, 8).map((model, i) => {
		const spend = spendByModel.get(model.label);
		return {
			label: model.label,
			color: ANALYTICS_COLORS[i % ANALYTICS_COLORS.length],
			pct: spend && spend.first !== 0 ? ((spend.second - spend.first) / spend.first) * 100 : 0,
			cost: Number(model.cost_total),
			requests: model.request_count,
			tokens: tokenTotal(model),
			sparkline: sparklinePath(seriesByModel.get(model.label) ?? []),
			icon: modelIcon(model.label, model.id),
		};
	});
});

const trendingUsers = computed(() =>
	props.byUser.slice(0, 8).map((row, index) => {
		const cost = Number(row.cost_total);
		const base = Math.max(cost, 0.0001);
		const offset = (index % 4) * 0.06;
		return {
			id: row.id,
			label: row.label || '-',
			initial: (row.label || '?').slice(0, 1),
			cost,
			requests: row.request_count,
			tokens: tokenTotal(row),
			share: userTotalCost.value > 0 ? (cost / userTotalCost.value) * 100 : 0,
			sparkline: sparklinePath([0.52, 0.64, 0.58, 0.76, 0.7].map(factor => base * (factor + offset)).concat(base)),
		};
	}),
);

function trendClass(value: number): string {
	if (value < 0) return 'text-red-400';
	if (value > 0) return 'text-emerald-500';
	return 'text-muted-foreground';
}

function trendLabel(value: number): string {
	if (value === 0) return '0%';
	return `${value > 0 ? '↑' : '↓'} ${Math.abs(value).toFixed(0)}%`;
}
</script>
