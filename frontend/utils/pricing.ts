import type {PriceRange} from '~/types/generated/PriceRange';
import type {PriceRates} from '~/types/generated/PriceRates';
import type {PriceServiceTier} from '~/types/generated/PriceServiceTier';
import type {PriceSheet} from '~/types/generated/PriceSheet';
import type {PriceSummary} from '~/types/generated/PriceSummary';
import type {PriceTier} from '~/types/generated/PriceTier';
import type {PriceWeekday} from '~/types/generated/PriceWeekday';
import type {PriceWindow} from '~/types/generated/PriceWindow';

export const RATE_FIELDS = ['input', 'output', 'cache_read', 'cache_write', 'reasoning', 'input_audio', 'output_audio'] as const;
export type RateField = (typeof RATE_FIELDS)[number];
export const WEEKDAYS: PriceWeekday[] = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'];

type Translate = (key: string, args?: Record<string, string | number>) => string;

export type RateDraft = Record<RateField, string>;
export type TierDraft = {min_context_tokens: string; rates: RateDraft};
export type ServiceTierDraft = {name: string; rates: RateDraft; tiers: PriceTier[]};
export type WindowDraft = {days: PriceWeekday[]; start: string; end: string; timezone: string; rates: RateDraft};
export type PricingDraft = {
	rates: RateDraft;
	tiers: TierDraft[] | null;
	service_tiers: ServiceTierDraft[] | null;
	windows: WindowDraft[] | null;
};

export function formatRate(value: number | null | undefined): string {
	return value == null ? '–' : `$${Number(value.toFixed(4))}`;
}

export function formatRange(range: PriceRange | null | undefined): string {
	if (!range) return '–';
	return range.min === range.max ? formatRate(range.min) : `${formatRate(range.min)}–${formatRate(range.max)}`;
}

export function hasSpecialRates(summary: PriceSummary | null | undefined): boolean {
	return !!summary && (summary.context_tiers.length > 0 || summary.service_tiers.length > 0 || summary.windows > 0);
}

function formatTokens(tokens: number): string {
	return tokens.toLocaleString();
}

function ratePair(rates: PriceRates, t: Translate): string {
	const parts: string[] = [];
	if (rates.input != null) parts.push(t('pricing.rate_in', {rate: formatRate(rates.input)}));
	if (rates.output != null) parts.push(t('pricing.rate_out', {rate: formatRate(rates.output)}));
	if (rates.cache_read != null) parts.push(t('pricing.rate_cache_read', {rate: formatRate(rates.cache_read)}));
	return parts.join(' · ') || t('pricing.rate_inherit');
}

function formatDays(days: PriceWeekday[], t: Translate): string {
	if (days.length === 0 || days.length === 7) return t('pricing.days_every');
	return days.map(day => t(`pricing.day_short.${day}`)).join(', ');
}

export function describeTier(tier: PriceTier, t: Translate): string {
	return `${t('pricing.tier_from', {tokens: formatTokens(tier.min_context_tokens)})}: ${ratePair(tier, t)}`;
}

export function describeWindow(window: PriceWindow, t: Translate): string {
	return `${formatDays(window.days, t)} ${window.start}–${window.end} ${window.timezone}: ${ratePair(window.rates, t)}`;
}

export function describeSheet(sheet: PriceSheet, t: Translate): string[] {
	const lines = (sheet.tiers ?? []).map(tier => describeTier(tier, t));
	for (const [name, service] of Object.entries(sheet.service_tiers ?? {})) {
		lines.push(`${t('pricing.service_tier_named', {name})}: ${ratePair(service, t)}`);
		for (const tier of service.tiers) lines.push(`  ${describeTier(tier, t)}`);
	}
	return lines.concat((sheet.windows ?? []).map(window => describeWindow(window, t)));
}

export function describeSummary(summary: PriceSummary, t: Translate): string[] {
	const lines: string[] = [];
	if (summary.context_tiers.length > 0) lines.push(t('pricing.summary_context', {tokens: summary.context_tiers.map(formatTokens).join(', ')}));
	if (summary.service_tiers.length > 0) lines.push(t('pricing.summary_service', {names: summary.service_tiers.join(', ')}));
	if (summary.windows > 0) lines.push(t('pricing.summary_windows', {count: summary.windows}));
	return lines;
}

function rateDraft(rates?: PriceRates | null): RateDraft {
	return Object.fromEntries(RATE_FIELDS.map(field => [field, rates?.[field] != null ? String(rates[field]) : ''])) as RateDraft;
}

export function emptyRateDraft(): RateDraft {
	return rateDraft(null);
}

export function tiersToDraft(tiers: PriceTier[]): TierDraft[] {
	return tiers.map(tier => ({min_context_tokens: String(tier.min_context_tokens), rates: rateDraft(tier)}));
}

export function serviceTiersToDraft(services: {[key in string]?: PriceServiceTier}): ServiceTierDraft[] {
	return Object.entries(services).map(([name, service]) => ({name, rates: rateDraft(service), tiers: service?.tiers ?? []}));
}

export function windowsToDraft(windows: PriceWindow[]): WindowDraft[] {
	return windows.map(window => ({days: [...window.days], start: window.start, end: window.end, timezone: window.timezone, rates: rateDraft(window.rates)}));
}

export function draftFromSheet(sheet: PriceSheet | null | undefined): PricingDraft {
	return {
		rates: rateDraft(sheet),
		tiers: sheet?.tiers ? tiersToDraft(sheet.tiers) : null,
		service_tiers: sheet?.service_tiers ? serviceTiersToDraft(sheet.service_tiers) : null,
		windows: sheet?.windows ? windowsToDraft(sheet.windows) : null,
	};
}

export class PricingDraftError extends Error {}

function parseRate(value: string | number | null | undefined): number | undefined {
	if (value == null || String(value).trim() === '') return undefined;
	const rate = Number(value);
	if (!Number.isFinite(rate) || rate < 0) throw new PricingDraftError('settings.models.editor.pricing_invalid');
	return rate;
}

function ratesFromDraft(draft: RateDraft): PriceRates {
	const rates: PriceRates = {};
	for (const field of RATE_FIELDS) {
		const rate = parseRate(draft[field]);
		if (rate !== undefined) rates[field] = rate;
	}
	return rates;
}

function tierFromDraft(draft: TierDraft): PriceTier {
	const threshold = Number(draft.min_context_tokens);
	if (!Number.isInteger(threshold) || threshold <= 0) throw new PricingDraftError('pricing.error.zero_threshold');
	return {min_context_tokens: threshold, ...ratesFromDraft(draft.rates)};
}

export function sheetFromDraft(draft: PricingDraft): PriceSheet {
	const sheet: PriceSheet = ratesFromDraft(draft.rates);
	if (draft.tiers) sheet.tiers = draft.tiers.map(tierFromDraft);
	if (draft.service_tiers) {
		sheet.service_tiers = Object.fromEntries(draft.service_tiers.map(service => [service.name.trim().toLowerCase(), {...ratesFromDraft(service.rates), tiers: service.tiers}]));
	}
	if (draft.windows) {
		sheet.windows = draft.windows.map(window => ({days: window.days, start: window.start, end: window.end, timezone: window.timezone.trim() || 'UTC', rates: ratesFromDraft(window.rates)}));
	}
	return sheet;
}
