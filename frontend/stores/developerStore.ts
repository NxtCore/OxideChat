import {defineStore} from 'pinia';
import type {CreateKeyPayload, DeveloperKey, DeveloperKeyBudget, DeveloperKeyRotation, DeveloperKeyRule, DeveloperKeyWithToken, DeveloperRequestLog, DeveloperTeam, RequestLogFilters, SetKeyBudgetPayload} from '~/types/developer';

interface Paginated<T> {
	has_more: boolean;
	items: T[];
}

export const useDeveloperStore = defineStore('developer', {
	state: () => ({
		teams: [] as DeveloperTeam[],
		keys: [] as DeveloperKey[],
		loading: false,
		loaded: false,
		sessionTokens: {} as Record<string, string>,
		error: null as string | null,
	}),
	getters: {
		activeKeys(state): DeveloperKey[] {
			return state.keys.filter(key => keyStatus(key) === 'active');
		},
		teamById: state => (id: string): DeveloperTeam | undefined => state.teams.find(team => team.id === id),
	},
	actions: {
		async load() {
			const {$customFetch} = useNuxtApp();
			this.loading = true;
			this.error = null;
			try {
				const [teams, keys] = await Promise.all([$customFetch<DeveloperTeam[]>('/api/v1/developer/teams'), $customFetch<DeveloperKey[]>('/api/v1/developer/keys')]);
				this.teams = teams ?? [];
				this.keys = keys ?? [];
				this.loaded = true;
			} catch (e: any) {
				this.error = errorMessage(e, '');
				throw e;
			} finally {
				this.loading = false;
			}
		},
		replaceKey(key: DeveloperKey) {
			const index = this.keys.findIndex(existing => existing.id === key.id);
			if (index === -1) this.keys.unshift(key);
			else this.keys[index] = key;
		},
		async createKey(payload: CreateKeyPayload): Promise<DeveloperKeyWithToken> {
			const {$customFetch} = useNuxtApp();
			const created = await $customFetch<DeveloperKeyWithToken>('/api/v1/developer/keys', {method: 'POST', body: payload});
			const {token, ...key} = created;
			this.sessionTokens[key.id] = token;
			this.replaceKey(key);
			return created;
		},
		async updateKey(id: string, patch: Record<string, unknown>) {
			const {$customFetch} = useNuxtApp();
			const key = await $customFetch<DeveloperKey>(`/api/v1/developer/keys/${id}`, {method: 'PATCH', body: patch});
			this.replaceKey(key);
			return key;
		},
		async revokeKey(id: string) {
			const {$customFetch} = useNuxtApp();
			const key = await $customFetch<DeveloperKey>(`/api/v1/developer/keys/${id}/revoke`, {method: 'POST'});
			this.replaceKey(key);
			return key;
		},
		async rotateKey(id: string, overlapSeconds: number): Promise<DeveloperKeyRotation> {
			const {$customFetch} = useNuxtApp();
			const rotation = await $customFetch<DeveloperKeyRotation>(`/api/v1/developer/keys/${id}/rotate`, {method: 'POST', body: {overlap_seconds: overlapSeconds}});
			const {token, ...key} = rotation.key;
			this.sessionTokens[key.id] = token;
			this.replaceKey(rotation.previous);
			this.replaceKey(key);
			return rotation;
		},
		async getLimits(id: string): Promise<DeveloperKeyRule[]> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<DeveloperKeyRule[]>(`/api/v1/developer/keys/${id}/limits`)) ?? [];
		},
		async setLimits(id: string, rules: DeveloperKeyRule[]): Promise<DeveloperKeyRule[]> {
			const {$customFetch} = useNuxtApp();
			const body = {
				rules: rules.map(({route_class, metric, capacity, refill_period_seconds}) => ({route_class, metric, capacity, refill_period_seconds})),
			};
			return (await $customFetch<DeveloperKeyRule[]>(`/api/v1/developer/keys/${id}/limits`, {method: 'PUT', body})) ?? [];
		},
		async getBudget(id: string): Promise<DeveloperKeyBudget | null> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<DeveloperKeyBudget | null>(`/api/v1/developer/keys/${id}/budget`)) ?? null;
		},
		async setBudget(id: string, payload: SetKeyBudgetPayload): Promise<DeveloperKeyBudget | null> {
			const {$customFetch} = useNuxtApp();
			return (await $customFetch<DeveloperKeyBudget | null>(`/api/v1/developer/keys/${id}/budget`, {method: 'PUT', body: payload})) ?? null;
		},
		async clearBudget(id: string) {
			const {$customFetch} = useNuxtApp();
			await $customFetch(`/api/v1/developer/keys/${id}/budget`, {method: 'DELETE'});
		},
		async loadRequestLogs(filters: RequestLogFilters, page = 1, limit = 50): Promise<Paginated<DeveloperRequestLog>> {
			const {$customFetch} = useNuxtApp();
			const params = Object.fromEntries(Object.entries({...filters, page, limit}).filter(([, value]) => value !== null && value !== undefined && value !== ''));
			return (await $customFetch<Paginated<DeveloperRequestLog>>('/api/v1/developer/request-logs', {params})) ?? {has_more: false, items: []};
		},
		async exportRequestLogs(filters: RequestLogFilters): Promise<DeveloperRequestLog[]> {
			const {$customFetch} = useNuxtApp();
			const params = Object.fromEntries(Object.entries(filters).filter(([, value]) => value !== null && value !== undefined && value !== ''));
			return (await $customFetch<DeveloperRequestLog[]>('/api/v1/developer/request-logs/export', {params})) ?? [];
		},
		async getRequestLogRetention(): Promise<number> {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch<{retention_days: number}>('/api/v1/developer/request-logs/retention');
			return response?.retention_days ?? 30;
		},
		async setRequestLogRetention(days: number): Promise<number> {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch<{retention_days: number}>('/api/v1/developer/request-logs/retention', {
				method: 'PUT',
				body: {retention_days: days},
			});
			return response?.retention_days ?? days;
		},
	},
});
