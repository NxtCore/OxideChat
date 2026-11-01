import type {DeveloperRequestLog} from '~/types/developer';

export type {AdminKey} from './generated/AdminKey';
export type {AdminKeyDetail} from './generated/AdminKeyDetail';
export type {AdminTeam} from './generated/AdminTeam';
export type {AuditEntry} from './generated/AuditEntry';
export type {GatewayEvent} from './generated/GatewayEvent';
export type {AdminUsageSummary} from './generated/AdminUsageSummary';
export type {Page} from './generated/Page';

export const ADMIN_KEY_STATUSES = ['active', 'disabled', 'expired', 'revoked'] as const;

export type AdminKeyStatus = (typeof ADMIN_KEY_STATUSES)[number];

export const AUDIT_CATEGORIES = ['key', 'limit', 'budget', 'model_policy', 'access'] as const;

export type AuditCategory = (typeof AUDIT_CATEGORIES)[number];

export interface AdminKeyFilters {
	team_id: string | null;
	status: AdminKeyStatus | null;
	expires_to: string | null;
	last_used_before: string | null;
	never_used: boolean | null;
	search: string | null;
}

export interface AuditFilters {
	category: AuditCategory | null;
	from: string | null;
	to: string | null;
}

export type AdminRequestLog = DeveloperRequestLog;
