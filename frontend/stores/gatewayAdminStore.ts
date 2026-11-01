import {defineStore} from 'pinia';
import type {DeveloperKeyRule} from '~/types/developer';
import type {AdminKey, AdminKeyDetail, AdminKeyFilters, AdminRequestLog, AdminTeam, AuditEntry, AuditFilters, GatewayEvent, Page} from '~/types/gatewayAdmin';

const BASE = '/api/v1/admin/gateway';

function clean(values: Record<string, unknown>): Record<string, unknown> {
	return Object.fromEntries(Object.entries(values).filter(([, value]) => value !== null && value !== undefined && value !== ''));
}

export const useGatewayAdminStore = defineStore('gatewayAdmin', {
	actions: {
		async listKeys(filters: AdminKeyFilters, page = 1): Promise<Page<AdminKey>> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<Page<AdminKey>>(`${BASE}/keys`, {params: clean({...filters, page})})) ?? {has_more: false, items: []};
		},
		async getKey(id: string): Promise<AdminKeyDetail> {
			const {$customFetch} = useNuxtApp();
			return await $customFetch<AdminKeyDetail>(`${BASE}/keys/${id}`);
		},
		async keyRequests(id: string): Promise<Page<AdminRequestLog>> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<Page<AdminRequestLog>>(`${BASE}/keys/${id}/requests`, {params: {size: 10}})) ?? {has_more: false, items: []};
		},
		async keyAction(id: string, action: 'revoke' | 'disable' | 'enable', reason: string): Promise<AdminKey> {
			const {$customFetch} = useNuxtApp();
			return await $customFetch<AdminKey>(`${BASE}/keys/${id}/${action}`, {method: 'POST', body: {reason: reason || null}});
		},
		async listTeams(search = '', page = 1): Promise<Page<AdminTeam>> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<Page<AdminTeam>>(`${BASE}/teams`, {params: clean({search, page, size: 100})})) ?? {has_more: false, items: []};
		},
		async setTeamLimits(id: string, rules: DeveloperKeyRule[], maxConcurrent: number | null, reason: string): Promise<AdminTeam> {
			const {$customFetch} = useNuxtApp();
			const body = {
				rules: rules.map(({route_class, metric, capacity, refill_period_seconds}) => ({route_class, metric, capacity, refill_period_seconds})),
				max_concurrent_requests: maxConcurrent,
				reason: reason || null,
			};
			return await $customFetch<AdminTeam>(`${BASE}/teams/${id}/limits`, {method: 'PUT', body});
		},
		async setTeamAccess(id: string, enabled: boolean, reason: string): Promise<AdminTeam> {
			const {$customFetch} = useNuxtApp();
			return await $customFetch<AdminTeam>(`${BASE}/teams/${id}/access`, {method: 'PUT', body: {enabled, reason: reason || null}});
		},
		async listAudit(filters: AuditFilters, page = 1): Promise<Page<AuditEntry>> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<Page<AuditEntry>>(`${BASE}/audit`, {params: clean({...filters, page})})) ?? {has_more: false, items: []};
		},
		async listEvents(days: number, threshold: number): Promise<GatewayEvent[]> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<GatewayEvent[]>(`${BASE}/events`, {params: {days, threshold_percent: threshold}})) ?? [];
		},
	},
});
