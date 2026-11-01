import type {ModelListAdmin, PaginatedResponse} from '~/types/chat';

export const modelsPath = '/api/v1/models';
export const adminModelsPath = '/api/v1/admin/models';
export const adminImageModelsPath = '/api/v1/admin/image-models';
export const adminImageModelProvidersPath = '/api/v1/admin/image-model-providers';

export const adminModels = (params: Record<string, string>) => useNuxtApp().$customFetch<PaginatedResponse<ModelListAdmin>>(adminModelsPath, {params});

export const adminModel = (id: string) => useNuxtApp().$customFetch(`${adminModelsPath}/${id}`);

export const patchAdminModel = (id: string, body: object) => useNuxtApp().$customFetch(`${adminModelsPath}/${id}`, {method: 'PATCH', body});

export const modelPricing = (id: string) => useNuxtApp().$customFetch(`${adminModelsPath}/${id}/pricing`);

export const saveModelPricing = (id: string, body: object) => useNuxtApp().$customFetch(`${adminModelsPath}/${id}/pricing`, {method: 'PUT', body});

export const deleteModelPricing = (id: string) => useNuxtApp().$customFetch(`${adminModelsPath}/${id}/pricing`, {method: 'DELETE'});

export const adminProviderOptions = (id: string, refresh = false) =>
	useNuxtApp().$customFetch(`${adminModelsPath}/${id}/provider-options${refresh ? '/refresh' : ''}`, {method: refresh ? 'POST' : 'GET'});

export const modelProviderOptions = <T>(id: string) => useNuxtApp().$customFetch<T>(`${modelsPath}/${id}/provider-options`);
