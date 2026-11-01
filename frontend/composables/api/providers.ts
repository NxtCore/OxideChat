export const adminProviders = () => useNuxtApp().$customFetch('/api/v1/admin/providers');

export const providerCatalog = (id: string, params: Record<string, string>) =>
	useNuxtApp().$customFetch<{has_more: boolean; items: any[]}>(`/api/v1/admin/providers/${id}/catalog`, {params});

export const saveProvider = (id: string | undefined, body: object) =>
	useNuxtApp().$customFetch(id ? `/api/v1/admin/providers/${id}` : '/api/v1/admin/providers', {method: id ? 'PUT' : 'POST', body});

export const deleteProvider = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/providers/${id}`, {method: 'DELETE'});

export const syncProvider = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/providers/${id}/sync`, {method: 'POST'});

export const testSavedProvider = <T>(id: string) => useNuxtApp().$customFetch<T>(`/api/v1/admin/providers/${id}/test`, {method: 'POST'});

export const testProviderConfig = <T>(body: object) => useNuxtApp().$customFetch<T>('/api/v1/admin/providers/test', {method: 'POST', body});
