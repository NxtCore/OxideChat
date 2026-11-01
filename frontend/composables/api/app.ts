export const patchAdminConfig = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/config', {method: 'PATCH', body});

export const adminConfig = <T>() => useNuxtApp().$customFetch<T>('/api/v1/admin/config');

export const adminTranslations = <T>() => useNuxtApp().$customFetch<T>('/api/v1/admin/i18n');

export const saveTranslation = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/i18n/translations', {method: 'PUT', body});

export const mySessions = <T>() => useNuxtApp().$customFetch<T>('/api/v1/users/@me/sessions');

export const revokeMySession = (id: string) => useNuxtApp().$customFetch(`/api/v1/users/@me/sessions/${id}`, {method: 'DELETE'});

export const checkHealth = () => useNuxtApp().$customFetch('/api/v1/health');

export const meAnalytics = <T>(params: object) => useNuxtApp().$customFetch<T[]>('/api/v1/me/analytics', {params});

export const oauthUrl = (provider: string) => `/api/v1/auth/oauth/${provider}`;
