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
	user_id: string | null;
	service_account: boolean;
	key_prefix: string;
	last_four: string;
	scopes: string[];
	is_enabled: boolean;
	admin_disabled: boolean;
	expires_at: string | null;
	revoked_at: string | null;
	last_used_at: string | null;
	max_concurrent_requests: number | null;
	created_at: string;
}

export interface DeveloperManagedTeam {
	id: string;
	name: string;
}

export interface DeveloperTeamKey extends DeveloperKey {
	team_name: string;
	owner_username: string | null;
}

export interface DeveloperTeamKeys {
	teams: DeveloperManagedTeam[];
	keys: DeveloperTeamKey[];
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
	service_account?: boolean;
}

export type KeyStatus = 'active' | 'disabled' | 'expired' | 'revoked';

export const KEY_SCOPES = ['inference:read', 'inference:write'] as const;

export type RequestLogStatus = 'success' | 'error';

export const REQUEST_LOG_ERROR_CATEGORIES = ['upstream', 'interrupted', 'budget', 'rate_limit', 'auth', 'model_access', 'internal'] as const;

export type RequestLogErrorCategory = (typeof REQUEST_LOG_ERROR_CATEGORIES)[number];

export interface DeveloperRequestLog {
	id: string;
	user_id: string;
	team_id: string | null;
	api_key_id: string | null;
	model_id: string | null;
	public_model_id: string;
	request_type: string;
	status: RequestLogStatus;
	status_code: number | null;
	input_tokens: number;
	output_tokens: number;
	reasoning_tokens: number;
	cost_total: string;
	created_at: string;
	request_id: string | null;
	provider: string | null;
	error_category: RequestLogErrorCategory | null;
	streaming: boolean;
	latency_ms: number | null;
	finish_reason: string | null;
	time_to_first_token_ms: number | null;
}

export interface RequestLogFilters {
	scope: 'team_service' | null;
	team_id: string | null;
	api_key_id: string | null;
	status: RequestLogStatus | null;
	error_category: RequestLogErrorCategory | null;
	model: string | null;
	provider: string | null;
	from: string | null;
	to: string | null;
}

export type BudgetInterval = 'daily' | 'weekly' | 'monthly' | 'total';
export type BudgetOnExceed = 'block' | 'warn';

export interface DeveloperKeyBudget {
	amount: string;
	interval: BudgetInterval;
	on_exceed: BudgetOnExceed;
	spent: string;
	remaining: string;
	window_start: string;
	resets_at: string | null;
	exhausted: boolean;
}

export interface SetKeyBudgetPayload {
	amount: number;
	interval: BudgetInterval;
	on_exceed: BudgetOnExceed;
}
