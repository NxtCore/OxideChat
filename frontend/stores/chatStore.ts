import {defineStore} from 'pinia';
import {useWorkspaceStore} from './workspaceStore';
import {useModelStore} from './modelStore';
import {usePreferencesStore} from './preferencesStore';
import type {Chat, ChatMessage, ChatWithMessages, CreateChatRequest, UpdateChatRequest, ChatListParams, PaginatedResponse} from '~/types/chat';

interface ChatState {
	chats: Chat[];
	activeChat: Chat | null;
	chatsLoading: boolean;

	messages: ChatMessage[];
	messagesLoading: boolean;

	isStreaming: boolean;

	pendingBranchContent: string | null;
	pendingBranchParts: any[] | null;

	initialized: boolean;
}

export const useChatStore = defineStore('chat', {
	state: (): ChatState => ({
		chats: [],
		activeChat: null,
		chatsLoading: false,

		messages: [],
		messagesLoading: false,

		isStreaming: false,

		pendingBranchContent: null,
		pendingBranchParts: null,

		initialized: false,
	}),

	getters: {
		pinnedChats(): Chat[] {
			return this.chats.filter(c => c.is_pinned && !c.is_archived);
		},

		recentChats(): Chat[] {
			return this.chats.filter(c => !c.is_pinned && !c.is_archived).sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
		},

		archivedChats(): Chat[] {
			return this.chats.filter(c => c.is_archived);
		},
	},

	actions: {
		async fetchChats(params?: ChatListParams) {
			this.chatsLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const query = new URLSearchParams();
				if (params?.workspace_id) query.set('workspace_id', params.workspace_id);
				if (params?.include_archived) query.set('include_archived', 'true');
				if (params?.page) query.set('page', params.page.toString());
				if (params?.size) query.set('size', params.size.toString());

				const url = `/api/v1/chats${query.toString() ? '?' + query.toString() : ''}`;
				const chats = await $customFetch<PaginatedResponse<Chat>>(url);
				this.chats = chats.items;
			} catch (e) {
				console.error('Failed to fetch chats:', e);
			} finally {
				this.chatsLoading = false;
			}
		},

		async createChat(data?: CreateChatRequest): Promise<Chat | null> {
			try {
				const router = useRouter();
				const {$customFetch} = useNuxtApp();
				const chat = await $customFetch('/api/v1/chats', {
					method: 'POST',
					body: data || {},
				});
				const newChat = chat as Chat;
				this.chats.unshift(newChat);
				this.activeChat = newChat;
				this.messages = [];
				router.push(`/chats/${newChat.id}`);
				return newChat;
			} catch (e) {
				console.error('Failed to create chat:', e);
				return null;
			}
		},

		async fetchChat(id: string, opts: { silent?: boolean } = {}): Promise<ChatWithMessages | null> {
			if (!opts.silent) this.messagesLoading = true;
			try {
			const {$customFetch} = useNuxtApp();
			const data = await $customFetch(`/api/v1/chats/${id}`);
			const chatWithMessages = data as ChatWithMessages;

			const existingById = new Map(this.messages.map(m => [m.id, m]));
			const reconciled: ChatMessage[] = [];

			for (const rawMsg of chatWithMessages.messages as any[]) {
				const toolCallsRecord: Record<string, any> = {};
				if (rawMsg.tool_calls && Array.isArray(rawMsg.tool_calls)) {
					for (const tc of rawMsg.tool_calls) {
						toolCallsRecord[tc.tool_call_id] = {
							name: tc.tool_name || '',
							args: typeof tc.input_args === 'string' ? tc.input_args : JSON.stringify(tc.input_args),
							output: tc.output,
							error: tc.error,
							isExecuting: false,
						};
					}
				}

				const existing = existingById.get(rawMsg.id) as any;
				if (existing) {
					const existingClientId = existing.client_id ?? rawMsg.id;
					Object.assign(existing, rawMsg);
					existing.client_id = existingClientId;
					if (Object.keys(toolCallsRecord).length) existing.toolCalls = toolCallsRecord;
					reconciled.push(existing);
				} else {
					const newMsg: any = {...rawMsg, client_id: rawMsg.id};
					if (Object.keys(toolCallsRecord).length) newMsg.toolCalls = toolCallsRecord;
					reconciled.push(newMsg);
				}
			}

				this.messages = reconciled;
				this.activeChat = chatWithMessages.chat;
				const modelStore = useModelStore();
				await modelStore.hydrateComposerFromMessages(this.messages);
				const lastAssistantMessage = this.messages.findLast(m => m.role === 'assistant');
				if (lastAssistantMessage)
					modelStore.setContextTokens((lastAssistantMessage.usage_details?.input_tokens || 0) + (lastAssistantMessage.usage_details?.output_tokens || 0));
				return chatWithMessages;
			} catch (e) {
				console.error('Failed to fetch chat:', e);
				return null;
			} finally {
				this.messagesLoading = false;
			}
		},

		async updateChat(id: string, data: UpdateChatRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const updated = await $customFetch(`/api/v1/chats/${id}`, {
					method: 'PATCH',
					body: data,
				});
				const index = this.chats.findIndex(c => c.id === id);
				if (index !== -1) this.chats[index] = updated as Chat;
				if (this.activeChat?.id === id) this.activeChat = updated as Chat;
				return true;
			} catch (e) {
				console.error('Failed to update chat:', e);
				return false;
			}
		},

		async deleteChat(id: string): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch(`/api/v1/chats/${id}`, {method: 'DELETE'});
				this.chats = this.chats.filter(c => c.id !== id);
				if (this.activeChat?.id === id) {
					this.activeChat = null;
					this.messages = [];
				}
				return true;
			} catch (e) {
				console.error('Failed to delete chat:', e);
				return false;
			}
		},

		async setActiveChat(chat: Chat | null) {
			if (chat) {
				this.fetchChat(chat.id);
			} else {
				const modelStore = useModelStore();
				this.messages = [];
				modelStore.setContextTokens(0);
				this.activeChat = null;
				if (this.initialized) await modelStore.resetComposerToDefaults();
			}
		},

		async editMessage(chatId: string, messageId: string, content: string): Promise<ChatMessage | null> {
			try {
				const {$customFetch} = useNuxtApp();
				const message = await $customFetch(`/api/v1/chats/${chatId}/messages/${messageId}/edit`, {
					method: 'POST',
					body: {content},
				});
				const newMsg = message as ChatMessage;
				await this.fetchChat(chatId, {silent: true});
				return newMsg;
			} catch (e) {
				console.error('Failed to edit message:', e);
				return null;
			}
		},

		async switchFork(chatId: string, messageId: string, forkIndex: number): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const message = await $customFetch(`/api/v1/chats/${chatId}/messages/${messageId}/switch-fork`, {
					method: 'POST',
					body: {fork_index: forkIndex},
				});
				await this.fetchChat(chatId, {silent: true});
				return true;
			} catch (e) {
				console.error('Failed to switch fork:', e);
				return false;
			}
		},

		async deleteFork(chatId: string, messageId: string): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch(`/api/v1/chats/${chatId}/messages/${messageId}/fork`, {
					method: 'DELETE',
				});
				this.messages = this.messages.filter(m => m.id !== messageId);
				return true;
			} catch (e) {
				console.error('Failed to delete fork:', e);
				return false;
			}
		},

		async branchFromMessage(chatId: string, messageId: string): Promise<Chat | null> {
			try {
				const router = useRouter();
				const {$customFetch} = useNuxtApp();
				const response = await $customFetch(`/api/v1/chats/${chatId}/messages/${messageId}/branch`, {
					method: 'POST',
					body: {},
				});
				const branchResult = response as {chat: Chat; prefill_content?: string; prefill_parts?: any[]};
				const newChat = branchResult.chat;
				this.chats.unshift(newChat);

				if (branchResult.prefill_content || branchResult.prefill_parts) {
					this.pendingBranchContent = branchResult.prefill_content || null;
					this.pendingBranchParts = branchResult.prefill_parts || null;
				}

				router.push(`/chats/${newChat.id}`);
				return newChat;
			} catch (e) {
				console.error('Failed to branch from message:', e);
				return null;
			}
		},

		clearPendingBranch() {
			this.pendingBranchContent = null;
			this.pendingBranchParts = null;
		},

		async init() {
			const workspaceStore = useWorkspaceStore();
			const modelStore = useModelStore();
			const preferencesStore = usePreferencesStore();
			this.initialized = false;
			workspaceStore.restoreActive();
			await Promise.all([workspaceStore.fetchWorkspaces(), preferencesStore.fetchPreferences(), preferencesStore.fetchUserMcpServers()]);
			workspaceStore.clearActiveIfMissing();
			await this.fetchChats({workspace_id: workspaceStore.activeWorkspaceId || undefined});
			workspaceStore.applyWorkspaceAccent();
			await modelStore.fetchModels();
			if (!this.activeChat) await modelStore.resetComposerToDefaults();
			this.initialized = true;
		},

		async refreshSharedData() {
			const modelStore = useModelStore();
			const preferencesStore = usePreferencesStore();
			await Promise.all([preferencesStore.fetchPreferences(), preferencesStore.fetchUserMcpServers()]);
			await modelStore.fetchModels();
			if (!this.activeChat) await modelStore.resetComposerToDefaults();
		},
	},
});
