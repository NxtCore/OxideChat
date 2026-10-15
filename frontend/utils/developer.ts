import type {DeveloperKey, KeyStatus} from '~/types/developer';

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
