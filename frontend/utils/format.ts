import {useMainStore} from '~/stores';

export function errorMessage(error: any, fallback: string): string {
	return error?.data?.message ?? error?.message ?? fallback;
}

export function formatMoney(value: string | number): string {
	const amount = Number(value);
	return `$${amount.toFixed(amount > 0 && amount < 0.01 ? 6 : 2)}`;
}

export function formatCost(value: string | number): string {
	const amount = Number(value);
	return amount > 0 && amount < 1 ? `$${amount.toFixed(4)}` : formatMoney(amount);
}

export function formatCurrency(value: string | number | null | undefined, currency: string): string {
	const parsed = Number(value ?? 0);
	return new Intl.NumberFormat(useMainStore().base?.language, {style: 'currency', currency: currency || 'USD'}).format(Number.isFinite(parsed) ? parsed : 0);
}

export function formatDateTime(value: string | null | undefined): string {
	return value ? new Date(value).toLocaleString(useMainStore().base?.language ?? 'en') : '–';
}

export function formatDate(value: string): string {
	return new Date(value).toLocaleDateString(useMainStore().base?.language ?? 'en', {dateStyle: 'medium'});
}
