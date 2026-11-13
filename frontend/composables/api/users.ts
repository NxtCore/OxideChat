import type {PaginatedResponse, UserListResponse} from '~/types/chat';
import type {UserOverviewResponse} from '~/types/users';

export const adminUsers = (params: Record<string, string | number>) => useNuxtApp().$customFetch<PaginatedResponse<UserListResponse>>('/api/v1/admin/users', {params});

export const createAdminUser = (body: object) => useNuxtApp().$customFetch('/api/v1/admin/users', {method: 'POST', body});

export const updateAdminUser = (id: string, body: object, part = '') => useNuxtApp().$customFetch(`/api/v1/admin/users/${id}${part}`, {method: 'PUT', body});

export const deleteAdminUser = (id: string) => useNuxtApp().$customFetch(`/api/v1/admin/users/${id}`, {method: 'DELETE'});

export const adminUserOverview = (id: string) => useNuxtApp().$customFetch<UserOverviewResponse>(`/api/v1/admin/users/${id}/overview`);

export const revokeAdminUserSession = (id: string, sessionId: string) => useNuxtApp().$customFetch(`/api/v1/admin/users/${id}/sessions/${sessionId}`, {method: 'DELETE'});
