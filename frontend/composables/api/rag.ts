import {useMainStore} from '~/stores';
import type {RagSetupResponse} from '~/types/generated/RagSetupResponse';
import type {ConnectionTestResponse} from '~/types/generated/ConnectionTestResponse';
import type {InspectRequest} from '~/types/generated/InspectRequest';
import type {InspectResponse} from '~/types/generated/InspectResponse';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import type {KnowledgeSearchResponse} from '~/types/generated/KnowledgeSearchResponse';
import type {SaveKnowledgeBaseRequest} from '~/types/generated/SaveKnowledgeBaseRequest';

const base = '/api/v1/admin/rag';

export const ragSetup = () => useNuxtApp().$customFetch<RagSetupResponse>(base);

export const saveRagSetup = (body: object) => useNuxtApp().$customFetch<RagSetupResponse>(base, {method: 'PUT', body});

export const createRagConnection = (body: object) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections`, {method: 'POST', body});

export const updateRagConnection = (id: string, body: object) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections/${id}`, {method: 'PUT', body});

export const deleteRagConnection = (id: string) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections/${id}`, {method: 'DELETE'});

export const testRagConnection = (id: string) => useNuxtApp().$customFetch<ConnectionTestResponse>(`${base}/connections/${id}/test`, {method: 'POST'});

export function ragErrorMessage(error: any, fallback: string): string {
	const reason = error?.data?.errors?.[0]?.details?.reason ?? error?.data?.details?.reason;
	return reason ? useMainStore().getTranslation(`rag.error.${reason}`) : errorMessage(error, fallback);
}

const knowledgeBase = '/api/v1/knowledge-bases';

export const knowledgeBases = () => useNuxtApp().$customFetch<KnowledgeBaseResponse[]>(knowledgeBase);

export const getKnowledgeBase = (id: string) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(`${knowledgeBase}/${id}`);

export const createKnowledgeBase = (body: SaveKnowledgeBaseRequest) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(knowledgeBase, {method: 'POST', body});

export const updateKnowledgeBase = (id: string, body: SaveKnowledgeBaseRequest) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(`${knowledgeBase}/${id}`, {method: 'PUT', body});

export const deleteKnowledgeBase = (id: string) => useNuxtApp().$customFetch(`${knowledgeBase}/${id}`, {method: 'DELETE'});

export const searchKnowledgeBase = (id: string, query: string) => useNuxtApp().$customFetch<KnowledgeSearchResponse>(`${knowledgeBase}/${id}/search`, {method: 'POST', body: {query}});

export const knowledgeAssignmentsPath = (id: string) => `${knowledgeBase}/${id}/assignments`;

export const inspectCollection = (connectionId: string, body: InspectRequest) => useNuxtApp().$customFetch<InspectResponse>(`${base}/connections/${connectionId}/inspect`, {method: 'POST', body});
