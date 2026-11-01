export const patchAdminConfig = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/config', {method: 'PATCH', body});

export const checkHealth = () => useNuxtApp().$customFetch('/api/v1/health');

export const meAnalytics = <T>(params: object) => useNuxtApp().$customFetch<T[]>('/api/v1/me/analytics', {params});

export const oauthUrl = (provider: string) => `/api/v1/auth/oauth/${provider}`;
