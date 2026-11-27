import {useMainStore} from '~/stores';
import type {AccessLabelsResponse} from '~/types/generated/AccessLabelsResponse';
import type {ConnectionTestResponse} from '~/types/generated/ConnectionTestResponse';
import type {EmbeddingSettings} from '~/types/generated/EmbeddingSettings';
import type {InspectRequest} from '~/types/generated/InspectRequest';
import type {InspectResponse} from '~/types/generated/InspectResponse';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import type {KnowledgeScopesResponse} from '~/types/generated/KnowledgeScopesResponse';
import type {KnowledgeSearchResponse} from '~/types/generated/KnowledgeSearchResponse';
import type {ModelAccessResponse} from '~/types/generated/ModelAccessResponse';
import type {ModelTargets} from '~/types/generated/ModelTargets';
import type {RagDocumentResponse} from '~/types/generated/RagDocumentResponse';
import type {RagSetupResponse} from '~/types/generated/RagSetupResponse';
import type {ReindexEstimateResponse} from '~/types/generated/ReindexEstimateResponse';
import type {SaveKnowledgeBaseRequest} from '~/types/generated/SaveKnowledgeBaseRequest';

const base = '/api/v1/admin/rag';

export const ragSetup = () => useNuxtApp().$customFetch<RagSetupResponse>(base);

export const saveRagSetup = (body: object) => useNuxtApp().$customFetch<RagSetupResponse>(base, {method: 'PUT', body});

export const createRagConnection = (body: object) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections`, {method: 'POST', body});

export const updateRagConnection = (id: string, body: object) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections/${id}`, {method: 'PUT', body});

export const deleteRagConnection = (id: string) => useNuxtApp().$customFetch<RagSetupResponse>(`${base}/connections/${id}`, {method: 'DELETE'});

export const testRagConnection = (id: string) => useNuxtApp().$customFetch<ConnectionTestResponse>(`${base}/connections/${id}/test`, {method: 'POST'});

export const reindexEstimate = (id: string, embedding: EmbeddingSettings) =>
	useNuxtApp().$customFetch<ReindexEstimateResponse>(`${base}/connections/${id}/reindex-estimate`, {method: 'POST', body: {embedding}});

export function ragErrorMessage(error: any, fallback: string): string {
	const details = error?.data?.errors?.[0]?.details ?? error?.data?.details;
	const reason = details?.reason;
	if (!reason) return errorMessage(error, fallback);
	if (reason === 'connection_model_conflict' && Array.isArray(details?.knowledge_bases)) {
		return useMainStore().getTranslation('rag.error.connection_model_conflict', {names: details.knowledge_bases.join(', ')});
	}
	return useMainStore().getTranslation(`rag.error.${reason}`);
}

export function ragDocErrorMessage(reason: string): string {
	return useMainStore().getTranslation(`rag.doc_error.${reason}`);
}

const knowledgeBase = '/api/v1/knowledge-bases';

export const knowledgeScopes = () => useNuxtApp().$customFetch<KnowledgeScopesResponse>(`${knowledgeBase}/scopes`);

export const knowledgeBases = () => useNuxtApp().$customFetch<KnowledgeBaseResponse[]>(knowledgeBase);

export const getKnowledgeBase = (id: string) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(`${knowledgeBase}/${id}`);

export const createKnowledgeBase = (body: SaveKnowledgeBaseRequest) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(knowledgeBase, {method: 'POST', body});

export const updateKnowledgeBase = (id: string, body: SaveKnowledgeBaseRequest) => useNuxtApp().$customFetch<KnowledgeBaseResponse>(`${knowledgeBase}/${id}`, {method: 'PUT', body});

export const deleteKnowledgeBase = (id: string) => useNuxtApp().$customFetch(`${knowledgeBase}/${id}`, {method: 'DELETE'});

export const searchKnowledgeBase = (id: string, query: string, asUser?: string | null) =>
	useNuxtApp().$customFetch<KnowledgeSearchResponse>(`${knowledgeBase}/${id}/search`, {method: 'POST', body: {query, ...(asUser ? {as_user: asUser} : {})}});

export const knowledgeAccessLabels = (id: string) => useNuxtApp().$customFetch<AccessLabelsResponse>(`${knowledgeBase}/${id}/access-labels`);

export const knowledgeLabelAssignmentsPath = (id: string, label: string) => `${knowledgeBase}/${id}/access-labels/assignments?label=${encodeURIComponent(label)}`;

export const knowledgeAssignmentsPath = (id: string) => `${knowledgeBase}/${id}/assignments`;

export const knowledgeModelAccess = (id: string) => useNuxtApp().$customFetch<ModelAccessResponse>(`${knowledgeBase}/${id}/model-access`);

export const saveKnowledgeModelAccess = (id: string, body: ModelTargets) =>
	useNuxtApp().$customFetch<ModelAccessResponse>(`${knowledgeBase}/${id}/model-access`, {method: 'PUT', body});

export const inspectCollection = (connectionId: string, body: InspectRequest) => useNuxtApp().$customFetch<InspectResponse>(`${base}/connections/${connectionId}/inspect`, {method: 'POST', body});

export const knowledgeDocuments = (id: string) => useNuxtApp().$customFetch<RagDocumentResponse[]>(`${knowledgeBase}/${id}/documents`);

export const uploadKnowledgeDocument = (id: string, file: File) =>
	useNuxtApp().$customFetch<RagDocumentResponse>(`${knowledgeBase}/${id}/documents?filename=${encodeURIComponent(file.name)}`, {
		method: 'POST',
		body: file,
		headers: {'Content-Type': 'application/octet-stream'},
	});

export const deleteKnowledgeDocument = (id: string, docId: string) => useNuxtApp().$customFetch(`${knowledgeBase}/${id}/documents/${docId}`, {method: 'DELETE'});

export const retryKnowledgeDocument = (id: string, docId: string) => useNuxtApp().$customFetch<RagDocumentResponse>(`${knowledgeBase}/${id}/documents/${docId}/retry`, {method: 'POST'});

export const documentAssignmentsPath = (id: string, docId: string) => `${knowledgeBase}/${id}/documents/${docId}/assignments`;
