import type {StoredFile} from '~/types/generated/StoredFile';

export type FilePurpose = 'image' | 'icon' | 'chat_attachment';

export const FILE_MAX_BYTES = 25 * 1024 * 1024;

export const ATTACHMENT_ACCEPT =
	'.txt,.md,.markdown,.csv,.tsv,.json,.jsonl,.yaml,.yml,.xml,.log,.ini,.toml,.sql,.html,.htm,.pdf,.rs,.py,.js,.mjs,.ts,.tsx,.jsx,.vue,.go,.java,.kt,.c,.h,.cpp,.hpp,.cs,.rb,.php,.sh,.css,.scss,.swift';

export const fileUrl = (id: string, base = '') => `${base}/api/v1/files/${encodeURIComponent(id)}`;

export const postFile = (file: File, purpose: FilePurpose) =>
	useNuxtApp().$customFetch<StoredFile>('/api/v1/files', {method: 'POST', query: {filename: file.name || purpose, purpose}, body: file});

export function fileErrorReason(error: any): string | undefined {
	return error?.data?.errors?.[0]?.details?.reason ?? error?.data?.details?.reason;
}

export function fileErrorMessage(error: any): string {
	const reason = fileErrorReason(error);
	return useMainStore().getTranslation(reason ? `files.error.${reason}` : 'files.error.upload');
}
