export const patchAdminConfig = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/config', {method: 'PATCH', body});

export const oauthProviders = <T>() => useNuxtApp().$customFetch<T[]>('/api/v1/admin/oauth-providers');

export const oauthPresets = <T>() => useNuxtApp().$customFetch<T[]>('/api/v1/admin/oauth-providers/presets');

export const createOauthProvider = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/oauth-providers', {method: 'POST', body});

export const updateOauthProvider = (id: string, body: object) => useNuxtApp().$customFetch(`/api/v1/admin/oauth-providers/${id}`, {method: 'PUT', body});

export const deleteOauthProvider = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/oauth-providers/${id}`, {method: 'DELETE'});

export const adminTranslations = <T>() => useNuxtApp().$customFetch<T>('/api/v1/admin/i18n');

export const saveTranslation = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/i18n/translations', {method: 'PUT', body});

export const mySessions = <T>() => useNuxtApp().$customFetch<T>('/api/v1/users/@me/sessions');

export const revokeMySession = (id: string) => useNuxtApp().$customFetch(`/api/v1/users/@me/sessions/${id}`, {method: 'DELETE'});

export const checkHealth = () => useNuxtApp().$customFetch('/api/v1/health');

export const meAnalytics = <T>(params: object) => useNuxtApp().$customFetch<T[]>('/api/v1/me/analytics', {params});

export const oauthUrl = (provider: string) => `/api/v1/auth/oauth/${provider}`;
