import {useIconsStore} from '@/stores/icons';
import {sanitizeSvg} from '@/composables/useMarkdown';
import type {AnalyticsDayModelRow, AnalyticsRow, ApiKeyAnalyticsRow} from '~/types/budgets';

export const ANALYTICS_COLORS = [
	'#ef4444', '#f97316', '#eab308', '#22c55e', '#06b6d4',
	'#3b82f6', '#8b5cf6', '#ec4899', '#f43f5e', '#14b8a6',
	'#a855f7', '#6366f1', '#d946ef', '#0ea5e9', '#84cc16',
];

export function formatTokens(n: number): string {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return String(n);
}

export function formatNumber(n: number): string {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
	return n.toLocaleString();
}

export function tokenTotal(row: Pick<AnalyticsRow, 'input_tokens' | 'output_tokens' | 'reasoning_tokens'>): number {
	return row.input_tokens + row.output_tokens + row.reasoning_tokens;
}

export function apiKeyLabel(row: ApiKeyAnalyticsRow): string {
	if (!row.id) return translateOr('settings.analytics.unknown_api_key', 'Unknown key');
	return `${row.name ?? ''} •••• ${row.last_four ?? ''}`;
}

export function sparklinePath(values: number[]): string {
	if (values.length < 2) return '';
	const min = Math.min(...values);
	const max = Math.max(...values);
	const range = max - min || 1;
	const w = 64;
	const h = 24;
	const padding = 2;
	const points = values.map((v, i) => {
		const x = padding + (i / (values.length - 1)) * (w - padding * 2);
		const y = h - padding - ((v - min) / range) * (h - padding * 2);
		return `${x.toFixed(1)},${y.toFixed(1)}`;
	});
	return `M${points.join('L')}`;
}

export function modelIcon(label: string, id: string | null) {
	const icon = useIconsStore().getProviderIcon(label, id ?? undefined);
	if (icon?.type === 'svg') return {...icon, icon: sanitizeSvg(icon.icon)};
	return icon;
}

export function stackedModelSeries(byModel: AnalyticsRow[], byDayModel: AnalyticsDayModelRow[]) {
	const topNames = byModel.slice(0, 8).map(r => r.label);
	const topSet = new Set(topNames);
	const dayMap = new Map<string, Record<string, number | string>>();
	for (const row of byDayModel) {
		const bucket = topSet.has(row.model_name) ? row.model_name : 'Other';
		const entry = dayMap.get(row.day) ?? {day: row.day};
		entry[bucket] = ((entry[bucket] as number) || 0) + Number(row.cost_total);
		dayMap.set(row.day, entry);
	}
	const categories: Record<string, {name: string; color: string}> = {};
	topNames.forEach((name, i) => {
		categories[name] = {name, color: ANALYTICS_COLORS[i % ANALYTICS_COLORS.length]};
	});
	if (byDayModel.some(r => !topSet.has(r.model_name))) categories['Other'] = {name: 'Other', color: '#6b7280'};
	return {data: Array.from(dayMap.values()), categories};
}
