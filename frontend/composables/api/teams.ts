import type {PaginatedResponse, TeamDetailed, TeamList} from '~/types/chat';

const teamUrl = (id: string, part = '') => `/api/v1/admin/teams/${id}${part}`;

export const adminTeams = (params: Record<string, string | number>) => useNuxtApp().$customFetch<PaginatedResponse<TeamList>>('/api/v1/admin/teams', {params});

export const adminTeam = (id: string) => useNuxtApp().$customFetch<TeamDetailed>(teamUrl(id));

export const createAdminTeam = (body: object) => useNuxtApp().$customFetch<TeamDetailed>('/api/v1/admin/teams', {method: 'POST', body});

export const putAdminTeam = (id: string, part: string, body: object) => useNuxtApp().$customFetch<TeamDetailed>(teamUrl(id, part), {method: 'PUT', body});

export const deleteAdminTeam = (id: string) => useNuxtApp().$customFetch(teamUrl(id), {method: 'DELETE'});
