import type {DeveloperKeyRule, DeveloperRequestLog} from '~/types/developer';

export const ADMIN_KEY_STATUSES = ['active', 'disabled', 'expired', 'revoked'] as const;

export type AdminKeyStatus = (typeof ADMIN_KEY_STATUSES)[number];

export const AUDIT_CATEGORIES = ['key', 'limit', 'budget', 'model_policy', 'access'] as const;

export type AuditCategory = (typeof AUDIT_CATEGORIES)[number];

export interface Page<T> {
	has_more: boolean;
	items: T[];
}

export interface AdminKey {
	id: string;
	name: string;
	team_id: string;
	team_name: string;
	user_id: string | null;
	owner_username: string | null;
	service_account: boolean;
	key_prefix: string;
	last_four: string;
	scopes: string[];
	status: AdminKeyStatus;
	is_enabled: boolean;
	admin_disabled: boolean;
	expires_at: string | null;
	revoked_at: string | null;
	last_used_at: string | null;
	max_concurrent_requests: number | null;
	created_at: string;
}

export interface AdminKeyFilters {
	team_id: string | null;
	status: AdminKeyStatus | null;
	expires_to: string | null;
	last_used_before: string | null;
	never_used: boolean | null;
	search: string | null;
}

export interface AdminBudget {
	amount: string;
	spent: string;
	remaining: string;
	on_exceed: string;
	exhausted: boolean;
	budget: {name: string; interval: string};
}

export interface AdminUsageSummary {
	total_requests: number;
	error_requests: number;
	requests_24h: number;
	input_tokens: number;
	output_tokens: number;
	cost_total: string;
	last_request_at: string | null;
}

export interface AdminKeyDetail {
	key: AdminKey;
	team_gateway_enabled: boolean;
	team_max_concurrent_requests: number | null;
	team_rules: DeveloperKeyRule[];
	key_rules: DeveloperKeyRule[];
	budgets: AdminBudget[];
	usage: AdminUsageSummary;
}

export interface AdminTeam {
	id: string;
	name: string;
	is_default: boolean;
	gateway_enabled: boolean;
	max_concurrent_requests: number | null;
	rules: DeveloperKeyRule[];
	key_count: number;
	active_key_count: number;
}

export interface AuditEntry {
	id: string;
	event: string;
	category: string;
	actor_id: string | null;
	actor_username: string | null;
	target_type: string | null;
	target_id: string | null;
	target_label: string | null;
	reason: string | null;
	before: Record<string, unknown> | null;
	after: Record<string, unknown> | null;
	metadata: Record<string, unknown>;
	created_at: string;
}

export interface AuditFilters {
	category: AuditCategory | null;
	from: string | null;
	to: string | null;
}

export interface GatewayEvent {
	kind: 'key_expiring' | 'budget_threshold';
	subject_type: 'key' | 'team';
	subject_id: string;
	subject_label: string;
	team_id: string | null;
	expires_at: string | null;
	amount: string | null;
	spent: string | null;
	percent_used: string | null;
}

export type AdminRequestLog = DeveloperRequestLog;
