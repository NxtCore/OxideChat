import {defineStore} from 'pinia';
import type {CreateKeyPayload, DeveloperKey, DeveloperKeyBudget, DeveloperKeyRotation, DeveloperKeyRule, DeveloperKeyWithToken, DeveloperTeam, SetKeyBudgetPayload} from '~/types/developer';

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
	},
});
