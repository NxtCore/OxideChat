export const adminTools = () => useNuxtApp().$customFetch('/api/v1/admin/tools');

export const userTools = () => useNuxtApp().$customFetch('/api/v1/tools');

export const saveAdminTool = (id: string | undefined, body: object) =>
	useNuxtApp().$customFetch(id ? `/api/v1/admin/tools/${id}` : '/api/v1/admin/tools', {method: id ? 'PUT' : 'POST', body});

export const deleteAdminTool = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/tools/${id}`, {method: 'DELETE'});

export const toolSettings = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/tools/${id}/settings`);

export const saveToolSettings = (id: string, body: object) => useNuxtApp().$customFetch(`/api/v1/admin/tools/${id}/settings`, {method: 'PUT', body});

export const testTool = <T>(id: string, body: object) => useNuxtApp().$customFetch<T>(`/api/v1/admin/tools/${id}/test`, {method: 'POST', body});

export const uploadWasm = (body: object) => useNuxtApp().$customFetch<{blob_id: string; size_bytes: number}>('/api/v1/admin/tools/wasm/upload', {method: 'POST', body});
