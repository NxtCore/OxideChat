import {useMainStore} from '~/stores';
import type {RagSetupResponse} from '~/types/generated/RagSetupResponse';
import type {ConnectionTestResponse} from '~/types/generated/ConnectionTestResponse';

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
