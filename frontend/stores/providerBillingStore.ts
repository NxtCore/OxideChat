import {defineStore} from 'pinia';
import type {AsyncState} from '~/composables/useAsyncAction';
import type {ProviderBillingOverview, UpdateProviderBillingPayload} from '~/types/providers';

export const useProviderBillingStore = defineStore('providerBilling', {
	state: () => ({
		overviews: {} as Record<string, ProviderBillingOverview>,
		states: {} as Record<string, AsyncState>,
		listLoading: false,
	}),
	actions: {
		async fetchBillingOverviews() {
			const {$customFetch} = useNuxtApp();
			this.listLoading = true;
			try {
				const rows = await $customFetch<ProviderBillingOverview[]>('/api/v1/admin/providers/billing');
				this.overviews = Object.fromEntries((rows ?? []).map(row => [row.provider_id, row]));
			} finally {
				this.listLoading = false;
			}
		},
		async fetchProviderBilling(providerId: string) {
			return this.run(providerId, () => useNuxtApp().$customFetch<ProviderBillingOverview>(`/api/v1/admin/providers/${providerId}/billing`));
		},
		async updateProviderBilling(providerId: string, payload: UpdateProviderBillingPayload) {
			return this.run(providerId, () => useNuxtApp().$customFetch<ProviderBillingOverview>(`/api/v1/admin/providers/${providerId}/billing`, {method: 'PUT', body: payload}));
		},
		async removeProviderBilling(providerId: string) {
			await this.run(providerId, () => useNuxtApp().$customFetch(`/api/v1/admin/providers/${providerId}/billing`, {method: 'DELETE'}), false);
			delete this.overviews[providerId];
		},
		async refreshProviderBilling(providerId: string) {
			return this.run(providerId, () => useNuxtApp().$customFetch<ProviderBillingOverview>(`/api/v1/admin/providers/${providerId}/billing/refresh`, {method: 'POST'}));
		},
		async run(providerId: string, action: () => Promise<any>, storeResult = true) {
			this.states[providerId] ??= {loading: false, error: null};
			const result = await trackAsync(this.states[providerId], action);
			if (storeResult && result) this.overviews[providerId] = result;
			return result;
		},
	},
});
