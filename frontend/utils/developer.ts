import type {DeveloperKey, DeveloperRequestLog, KeyStatus} from '~/types/developer';

export function keyStatus(key: DeveloperKey): KeyStatus {
	if (key.revoked_at) return 'revoked';
	if (key.expires_at && new Date(key.expires_at).getTime() <= Date.now()) return 'expired';
	if (!key.is_enabled) return 'disabled';
	return 'active';
}

export function maskedKey(key: DeveloperKey): string {
	return `${key.key_prefix}…${key.last_four}`;
}

export function errorMessage(error: any, fallback: string): string {
	return error?.data?.message ?? error?.message ?? fallback;
}

export function gatewayBaseUrl(): string {
	return `${window.location.origin}/openai/v1`;
}

export function formatMoney(value: string | number): string {
	const amount = Number(value);
	const small = amount > 0 && amount < 0.01;
	return `$${amount.toFixed(small ? 6 : 2)}`;
}

export function formatLatency(value: number | null): string {
	if (value === null) return '–';
	return value >= 1000 ? `${(value / 1000).toFixed(2)} s` : `${value} ms`;
}

export function requestLogsCsv(logs: DeveloperRequestLog[]): string {
	const headers = [
		'created_at',
		'public_model_id',
		'request_type',
		'status',
		'status_code',
		'error_category',
		'request_id',
		'provider',
		'streaming',
		'latency_ms',
		'input_tokens',
		'output_tokens',
		'reasoning_tokens',
		'cost_total',
		'team_id',
		'api_key_id',
		'model_id',
	];
	const escape = (value: string) => (value.includes(',') || value.includes('"') || value.includes('\n') ? '"' + value.replaceAll('"', '""') + '"' : value);
	const rows = logs.map(log =>
		[
			log.created_at,
			log.public_model_id,
			log.request_type,
			log.status,
			log.status_code ?? '',
			log.error_category ?? '',
			log.request_id ?? '',
			log.provider ?? '',
			log.streaming,
			log.latency_ms ?? '',
			log.input_tokens,
			log.output_tokens,
			log.reasoning_tokens,
			log.cost_total,
			log.team_id ?? '',
			log.api_key_id ?? '',
			log.model_id ?? '',
		]
			.map(value => escape(String(value)))
			.join(',')
	);
	return [headers.join(','), ...rows].join('\n');
}

export function downloadFile(filename: string, content: string, mimeType: string): void {
	const url = URL.createObjectURL(new Blob([content], {type: mimeType}));
	const link = document.createElement('a');
	link.href = url;
	link.download = filename;
	document.body.appendChild(link);
	link.click();
	link.remove();
	URL.revokeObjectURL(url);
}
