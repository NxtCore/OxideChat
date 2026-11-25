import type {ChatAttachment} from '~/types/generated/ChatAttachment';

export const ATTACHMENT_MAX_BYTES = 25 * 1024 * 1024;

export const ATTACHMENT_ACCEPT =
	'.txt,.md,.markdown,.csv,.tsv,.json,.jsonl,.yaml,.yml,.xml,.log,.ini,.toml,.sql,.html,.htm,.pdf,.rs,.py,.js,.mjs,.ts,.tsx,.jsx,.vue,.go,.java,.kt,.c,.h,.cpp,.hpp,.cs,.rb,.php,.sh,.css,.scss,.swift';

export const postAttachment = (file: File) =>
	useNuxtApp().$customFetch<ChatAttachment>('/api/v1/chat-attachments', {method: 'POST', query: {filename: file.name}, body: file});

export function attachmentErrorMessage(error: any): string {
	const reason = error?.data?.errors?.[0]?.details?.reason ?? error?.data?.details?.reason;
	return useMainStore().getTranslation(reason ? `chat.attachments.error.${reason}` : 'chat.attachments.error.upload');
}
