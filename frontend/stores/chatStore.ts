import {defineStore} from 'pinia';
import {useMainStore} from './index';
import type {
	Workspace,
	Chat,
	ChatMessage,
	ChatWithMessages,
	UserPreferences,
	Model,
	CreateWorkspaceRequest,
	UpdateWorkspaceRequest,
	CreateChatRequest,
	UpdateChatRequest,
	UpdatePreferencesRequest,
	ChatListParams,
	StreamingAnimation,
} from '~/types/chat';

interface ChatState {
	workspaces: Workspace[];
	activeWorkspaceId: string | null;
	workspacesLoading: boolean;

	chats: Chat[];
	activeChat: Chat | null;
	chatsLoading: boolean;

	messages: ChatMessage[];
	messagesLoading: boolean;

	models: Model[];
	selectedModel: Model | null;
	modelsLoading: boolean;

	isStreaming: boolean;
	contextTokens: number;
	reasoningEffort: string | null;
	reasoningBudget: number | null;
	enabledTools: string[];

	// Branch prefill state
	pendingBranchContent: string | null;
	pendingBranchParts: any[] | null;
}

export const useChatStore = defineStore('chat', {
	state: (): ChatState => ({
		workspaces: [],
		activeWorkspaceId: null,
		workspacesLoading: false,

		chats: [],
		activeChat: null,
		chatsLoading: false,

		messages: [],
		messagesLoading: false,

		models: [],
		selectedModel: null,
		modelsLoading: false,

		isStreaming: false,
		contextTokens: 0,
		reasoningEffort: null,
		reasoningBudget: null,
		enabledTools: [],

		pendingBranchContent: null,
		pendingBranchParts: null,
	}),

	getters: {
		defaultWorkspace(): Workspace | undefined {
			return this.workspaces.find(w => w.is_default);
		},

		activeWorkspace(): Workspace | undefined {
			if (!this.activeWorkspaceId) return this.defaultWorkspace;
			return this.workspaces.find(w => w.id === this.activeWorkspaceId);
		},

		pinnedChats(): Chat[] {
			return this.chats.filter(c => c.is_pinned && !c.is_archived);
		},

		recentChats(): Chat[] {
			return this.chats.filter(c => !c.is_pinned && !c.is_archived).sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
		},

		archivedChats(): Chat[] {
			return this.chats.filter(c => c.is_archived);
		},

		favoriteModels(): Model[] {
			const mainStore = useMainStore();
			return this.models.filter(m => m.is_favorite || mainStore.preferences?.favorite_model_keys.includes(m.model_id));
		},

		groupedModels(): Record<string, Model[]> {
			const grouped: Record<string, Model[]> = {};
			for (const model of this.models.filter(m => !m.is_hidden)) {
				const provider = model.provider_name;
				if (!grouped[provider]) grouped[provider] = [];
				grouped[provider].push(model);
			}
			return grouped;
		},

		hasReasoningCapability: state => (model: Model | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.some(c => c.startsWith('REASONING_'));
		},

		hasToolCapability: state => (model: Model | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.includes('TOOLS');
		},

		availableReasoningEfforts(): string[] {
			if (!this.selectedModel) return [];
			return this.selectedModel.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
		},

		hasNoneReasoningOption(): boolean {
			return this.availableReasoningEfforts.includes('NONE');
		},

		isReasoningRequired(): boolean {
			return this.hasReasoningCapability(null) && this.availableReasoningEfforts.length < 2;
		},

		lowestReasoningEffort(): string | null {
			const efforts = this.availableReasoningEfforts;
			if (efforts.length === 0) return null;
			const effortOrder = ['MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
			for (const effort of effortOrder) {
				if (efforts.includes(effort)) return effort;
			}
			return efforts[0] || null;
		},

		contextPercentage(): number {
			if (!this.selectedModel?.context_length) return 0;
			return Math.min(100, (this.contextTokens / this.selectedModel.context_length) * 100);
		},

		isFavoriteModel: state => (model: Model) => {
			const mainStore = useMainStore();
			return mainStore.preferences?.favorite_model_keys.includes(model.model_id);
		},
	},

	actions: {
		async fetchWorkspaces() {
			this.workspacesLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const workspaces = await $customFetch('/api/v1/workspaces');
				this.workspaces = workspaces as Workspace[];
			} catch (e) {
				console.error('Failed to fetch workspaces:', e);
			} finally {
				this.workspacesLoading = false;
			}
		},

		async createWorkspace(data: CreateWorkspaceRequest): Promise<Workspace | null> {
			try {
				const {$customFetch} = useNuxtApp();
				const workspace = await $customFetch('/api/v1/workspaces', {
					method: 'POST',
					body: data,
				});
				this.workspaces.push(workspace as Workspace);
				return workspace as Workspace;
			} catch (e) {
				console.error('Failed to create workspace:', e);
				return null;
			}
		},

		async updateWorkspace(id: string, data: UpdateWorkspaceRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const updated = await $customFetch(`/api/v1/workspaces/${id}`, {
					method: 'PATCH',
					body: data,
				});
				const index = this.workspaces.findIndex(w => w.id === id);
				if (index !== -1) this.workspaces[index] = updated as Workspace;
				return true;
			} catch (e) {
				console.error('Failed to update workspace:', e);
				return false;
			}
		},

		async deleteWorkspace(id: string): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch(`/api/v1/workspaces/${id}`, {method: 'DELETE'});
				this.workspaces = this.workspaces.filter(w => w.id !== id);
				if (this.activeWorkspaceId === id) this.activeWorkspaceId = null;
				return true;
			} catch (e) {
				console.error('Failed to delete workspace:', e);
				return false;
			}
		},

		setActiveWorkspace(id: string | null) {
			this.activeWorkspaceId = id;
			this.fetchChats({workspace_id: id || undefined});
		},

		async fetchChats(params?: ChatListParams) {
			this.chatsLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const query = new URLSearchParams();
				if (params?.workspace_id) query.set('workspace_id', params.workspace_id);
				if (params?.include_archived) query.set('include_archived', 'true');
				if (params?.limit) query.set('limit', params.limit.toString());
				if (params?.offset) query.set('offset', params.offset.toString());

				const url = `/api/v1/chats${query.toString() ? '?' + query.toString() : ''}`;
				const chats = await $customFetch(url);
				this.chats = chats as Chat[];
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

		async fetchChat(id: string): Promise<ChatWithMessages | null> {
			this.messagesLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const data = await $customFetch(`/api/v1/chats/${id}`);
				const chatWithMessages = data as ChatWithMessages;
				this.activeChat = chatWithMessages.chat;
				this.messages = chatWithMessages.messages.map(msg => {
					const rawMsg = msg as any;
					if (rawMsg.tool_calls && Array.isArray(rawMsg.tool_calls)) {
						const toolCallsRecord: Record<string, any> = {};
						for (const tc of rawMsg.tool_calls) {
							toolCallsRecord[tc.tool_call_id] = {
								name: tc.tool_name || '',
								args: typeof tc.input_args === 'string' ? tc.input_args : JSON.stringify(tc.input_args),
								output: tc.output,
								error: tc.error,
								isExecuting: false,
							};
						}
						return {...msg, toolCalls: toolCallsRecord};
					}
					return msg;
				});
				const lastAssistantMessage = this.messages.findLast(m => m.role === 'assistant');
				if (lastAssistantMessage)
					this.setContextTokens((lastAssistantMessage.usage_details?.input_tokens || 0) + (lastAssistantMessage.usage_details?.output_tokens || 0));
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

		setActiveChat(chat: Chat | null) {
			if (chat) {
				this.fetchChat(chat.id);
			} else {
				this.messages = [];
				this.setContextTokens(0);
				this.activeChat = null;
			}
		},

		async sendAndStream(chatId: string, content: string, parts?: any[], skipUserMessage: boolean = false, regenerateFromMessageId?: string): Promise<void> {
			if (!this.selectedModel) {
				console.error('No model selected');
				return;
			}

			this.isStreaming = true;

			let userMessageId: string | null = null;

			// Only create local user message if not skipping (for regeneration after edit)
			if (!skipUserMessage) {
				userMessageId = `user-${Date.now()}`;
				const userMessage: ChatMessage = {
					id: userMessageId,
					role: 'user',
					content,
					reasoning_content: null,
					model_id: this.selectedModel.model_id,
					cost_details: {
						input: null,
						output: null,
						reasoning: null,
					},
					usage_details: {
						input_tokens: null,
						output_tokens: null,
						reasoning_tokens: null,
						latency_ms: null,
						reasoning_latency_ms: null,
					},
					reasoning_details: {
						effort: this.reasoningEffort,
						budget_tokens: this.reasoningBudget,
					},
					tool_calls: [],
					created_at: new Date().toISOString(),
					parent_id: null,
					fork_index: 1,
					sibling_count: 1,
				};
				this.messages.push(userMessage);
			}

			const streamingMessageId = `streaming-${Date.now()}`;
			const streamingMessage: ChatMessage = {
				id: streamingMessageId,
				role: 'assistant',
				content: '',
				reasoning_content: null,
				model_id: this.selectedModel.model_id,
				cost_details: {
					input: null,
					output: null,
					reasoning: null,
				},
				usage_details: {
					input_tokens: null,
					output_tokens: null,
					reasoning_tokens: null,
					latency_ms: null,
					reasoning_latency_ms: null,
				},
				reasoning_details: {
					effort: this.reasoningEffort,
					budget_tokens: this.reasoningBudget,
				},
				tool_calls: [],
				created_at: new Date().toISOString(),
				parent_id: null,
				fork_index: 1,
				sibling_count: 1,
			};
			this.messages.push(streamingMessage);

			try {
				const config = useRuntimeConfig();
				const baseUrl = config.public.apiBase || '';

				const body: any = {
					content,
					model_key: this.selectedModel.model_id,
					reasoning_effort: this.reasoningEffort || undefined,
					reasoning_budget_tokens: this.reasoningBudget || undefined,
					tools_enabled: this.enabledTools.length > 0 ? this.enabledTools : undefined,
					skip_user_message: skipUserMessage,
					regenerate_from_message_id: regenerateFromMessageId,
				};

				if (parts && parts.length > 0) {
					body.parts = parts;
				}

				const response = await fetch(`${baseUrl}/api/v1/chats/${chatId}/stream`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
					},
					credentials: 'include',
					body: JSON.stringify(body),
				});

				if (!response.ok) {
					throw new Error(`Stream request failed: ${response.status}`);
				}

				const reader = response.body?.getReader();
				if (!reader) {
					throw new Error('No response body');
				}

				const decoder = new TextDecoder();
				let buffer = '';

				while (true) {
					const {done, value} = await reader.read();
					if (done) break;

					buffer += decoder.decode(value, {stream: true});

					const lines = buffer.split('\n');
					buffer = lines.pop() || '';

					for (const line of lines) {
						if (line.startsWith('data: ')) {
							const jsonStr = line.slice(6).trim();
							if (!jsonStr) continue;

							try {
								const data = JSON.parse(jsonStr);
								const userMsgIndex = this.messages.findIndex(m => m.id === userMessageId);
								const msgIndex = this.messages.findIndex(m => m.id === streamingMessageId);
								const msg = msgIndex !== -1 ? this.messages[msgIndex] : null;

								switch (data.type) {
									case 'user_message_saved':
										if (userMsgIndex !== -1) {
											const userMsg = this.messages[userMsgIndex];
											if (userMsg) {
												Object.assign(userMsg, data.message);
											}
										}
										break;
									case 'text_delta':
										if (msg) msg.content += data.content;
										break;
									case 'reasoning_delta':
										if (msg) {
											if (msg.reasoning_content === null) msg.reasoning_content = '';
											msg.reasoning_content += data.content;
										}
										break;
									case 'tool_call_start':
										if (msg) {
											const toolCall = {
												tool_call_id: data.id,
												tool_name: data.name,
												input_args: '',
											};
											msg.tool_calls.push(toolCall as any);
										}
										break;
									case 'tool_call_delta': {
										const toolCall = msg?.tool_calls?.find(tc => tc.tool_call_id === data.id);
										if (toolCall && typeof toolCall.input_args === 'string') {
											toolCall.input_args += data.args_delta;
										} else {
											const newToolCall = {
												tool_call_id: data.id,
												tool_name: data.name,
												input_args: data.args_delta,
											};
											if (msg) msg.tool_calls.push(newToolCall as any);
										}

										break;
									}
									case 'tool_call_end': {
										break;
									}
									case 'tool_result': {
										const toolCall = msg?.tool_calls?.find(tc => tc.tool_call_id === data.id);
										if (toolCall) {
											toolCall.output = data.output ? JSON.stringify(data.output) : null;
											toolCall.error = data.error || null;
										}
										break;
									}
									case 'tokens':
										if (msg) {
											msg.usage_details.input_tokens = data.input;
											msg.usage_details.output_tokens = data.output;
										}
										this.contextTokens = data.input + data.output;
										break;
									case 'done':
										if (msg) {
											Object.assign(msg, data.message);
										}
										this.isStreaming = false;

										const chatIndex = this.chats.findIndex(c => c.id === chatId);
										const chat = this.chats[chatIndex];
										if (chat) {
											chat.message_count += 2;
											chat.updated_at = new Date().toISOString();
										}
										break;
									case 'error':
										const store = useMainStore();
										store.toast(data.message, {type: 'error'});
										console.error('Stream error:', data.code, data.message);
										if (msg) msg.content = `Error: ${data.message}`;
										this.isStreaming = false;
										break;
								}
							} catch (e) {
								console.error('Failed to parse SSE event:', e);
							}
						}
					}
				}
			} catch (e) {
				console.error('Failed to stream:', e);
				this.isStreaming = false;
				this.messages = this.messages.filter(m => {
					if (m.id === streamingMessageId) return false;
					if (userMessageId && m.id === userMessageId) return false;
					return true;
				});
			}
		},

		async fetchPreferences() {
			try {
				const {$customFetch} = useNuxtApp();
				const mainStore = useMainStore();
				const prefs = await $customFetch('/api/v1/users/@me/preferences');
				mainStore.preferences = prefs as UserPreferences;
			} catch (e) {
				console.error('Failed to fetch preferences:', e);
			}
		},

		async updatePreferences(data: UpdatePreferencesRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const mainStore = useMainStore();
				const updated = await $customFetch('/api/v1/users/@me/preferences', {
					method: 'PATCH',
					body: data,
				});
				mainStore.preferences = updated as UserPreferences;
				return true;
			} catch (e) {
				console.error('Failed to update preferences:', e);
				return false;
			}
		},

		async fetchModels() {
			this.modelsLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const mainStore = useMainStore();
				const models = await $customFetch('/api/v1/models');
				this.models = models as Model[];

				if (!this.selectedModel && this.models.length > 0) {
					const defaultKey = mainStore.preferences?.default_model_key;
					const defaultModel = defaultKey ? this.models.find(m => m.model_id === defaultKey) : null;
					this.selectedModel = defaultModel || this.models[0] || null;
				}
			} catch (e) {
				console.error('Failed to fetch models:', e);
			} finally {
				this.modelsLoading = false;
			}
		},

		setSelectedModel(model: Model | null) {
			this.selectedModel = model;

			if (!model) {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
				return;
			}

			const hasNone = model.capabilities.some(c => c === 'REASONING_EFFORT_NONE');
			const hasReasoningCap = model.capabilities.some(c => c.startsWith('REASONING_'));

			if (!hasReasoningCap) {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
			} else if (!hasNone) {
				const efforts = model.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
				const effortOrder = ['MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
				const lowestEffort = effortOrder.find(e => efforts.includes(e));
				this.reasoningEffort = lowestEffort ? lowestEffort.toLowerCase() : null;
				this.reasoningBudget = null;
			} else {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
			}
		},

		async toggleFavoriteModel(modelKey: string): Promise<boolean> {
			const mainStore = useMainStore();
			const favorites = [...(mainStore.preferences?.favorite_model_keys || [])];
			const index = favorites.indexOf(modelKey);

			if (index === -1) {
				favorites.push(modelKey);
			} else {
				favorites.splice(index, 1);
			}

			return this.updatePreferences({favorite_model_keys: favorites});
		},

		setStreamingAnimation(animation: StreamingAnimation) {
			this.updatePreferences({streaming_animation: animation});
		},

		setReasoningEffort(effort: string | null, isTokenBudget = false) {
			if (isTokenBudget) {
				this.reasoningBudget = effort ? parseInt(effort) : null;
			} else {
				this.reasoningEffort = effort;
			}
		},

		toggleTool(toolName: string) {
			const index = this.enabledTools.indexOf(toolName);
			if (index === -1) {
				this.enabledTools.push(toolName);
			} else {
				this.enabledTools.splice(index, 1);
			}
		},

		setContextTokens(tokens: number) {
			this.contextTokens = tokens;
		},

		// Fork operations
		async editMessage(chatId: string, messageId: string, content: string): Promise<ChatMessage | null> {
			try {
				const {$customFetch} = useNuxtApp();
				const message = await $customFetch(`/api/v1/chats/${chatId}/messages/${messageId}/edit`, {
					method: 'POST',
					body: {content},
				});
				const newMsg = message as ChatMessage;
				await this.fetchChat(chatId);
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
				// Reload the chat to get correct fork path
				await this.fetchChat(chatId);
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
				// Remove the message from local state
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

				// If prefill data, store it for composer
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
			await Promise.all([this.fetchWorkspaces(), this.fetchChats(), this.fetchPreferences()]);
			await this.fetchModels();
		},
	},
});
