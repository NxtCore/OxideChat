export interface DeveloperTeam {
	id: string;
	name: string;
	is_default: boolean;
	models: string[];
}

export interface DeveloperKey {
	id: string;
	name: string;
	team_id: string;
	key_prefix: string;
	last_four: string;
	scopes: string[];
	is_enabled: boolean;
	expires_at: string | null;
	revoked_at: string | null;
	last_used_at: string | null;
	max_concurrent_requests: number | null;
	created_at: string;
}

export interface DeveloperKeyWithToken extends DeveloperKey {
	token: string;
}

export interface DeveloperKeyRotation {
	key: DeveloperKeyWithToken;
	previous: DeveloperKey;
}

export interface DeveloperKeyRule {
	id?: string;
	route_class: 'inference' | 'utility';
	metric: 'requests' | 'tokens';
	capacity: number;
	refill_period_seconds: number;
}

export interface CreateKeyPayload {
	name: string;
	team_id: string;
	scopes: string[];
	expires_at?: string | null;
	max_concurrent_requests?: number | null;
}

export type KeyStatus = 'active' | 'disabled' | 'expired' | 'revoked';

export const KEY_SCOPES = ['inference:read', 'inference:write'] as const;
