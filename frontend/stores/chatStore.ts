import {defineStore} from 'pinia';
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
	// Workspaces
	workspaces: Workspace[];
	activeWorkspaceId: string | null;
	workspacesLoading: boolean;

	// Chats
	chats: Chat[];
	activeChat: Chat | null;
	chatsLoading: boolean;

	// Messages
	messages: ChatMessage[];
	messagesLoading: boolean;

	// Models
	models: Model[];
	selectedModel: Model | null;
	modelsLoading: boolean;

	// Preferences
	preferences: UserPreferences;
	preferencesLoading: boolean;

	// UI State
	isStreaming: boolean;
	contextTokens: number;
	reasoningEffort: string | null;
	enabledTools: string[];
}

const defaultPreferences: UserPreferences = {
	default_model_key: null,
	favorite_model_keys: [],
	streaming_animation: 'fade',
	use_remend: true,
};

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

		preferences: {...defaultPreferences},
		preferencesLoading: false,

		isStreaming: false,
		contextTokens: 0,
		reasoningEffort: null,
		enabledTools: [],
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
			return this.models.filter(m => m.is_favorite || this.preferences.favorite_model_keys.includes(m.model_id));
		},

		groupedModels(): Record<string, Model[]> {
			const grouped: Record<string, Model[]> = {};
			for (const model of this.models.filter(m => !m.is_hidden)) {
				const provider = model.provider_display_name;
				if (!grouped[provider]) grouped[provider] = [];
				grouped[provider].push(model);
			}
			return grouped;
		},

		hasReasoningCapability(): boolean {
			if (!this.selectedModel) return false;
			return this.selectedModel.capabilities.some(c => c.startsWith('REASONING_'));
		},

		availableReasoningEfforts(): string[] {
			if (!this.selectedModel) return [];
			return this.selectedModel.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
		},

		hasNoneReasoningOption(): boolean {
			return this.availableReasoningEfforts.includes('NONE');
		},

		isReasoningRequired(): boolean {
			return this.hasReasoningCapability && !this.hasNoneReasoningOption;
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
	},

	actions: {
		// ===== Workspaces =====
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
			// Re-fetch chats for this workspace
			this.fetchChats({workspace_id: id || undefined});
		},

		// ===== Chats =====
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
				this.messages = chatWithMessages.messages;
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
			this.activeChat = chat;
			if (chat) {
				this.fetchChat(chat.id);
			} else {
				this.messages = [];
				this.setContextTokens(0);
			}
		},

		/**
		 * Send a message and stream the AI response.
		 * This is a unified action that saves the user message and streams the AI response.
		 */
		async sendAndStream(chatId: string, content: string): Promise<void> {
			if (!this.selectedModel) {
				console.error('No model selected');
				return;
			}

			this.isStreaming = true;

			// Create a placeholder user message
			const userMessageId = `user-${Date.now()}`;
			const userMessage: ChatMessage = {
				id: userMessageId,
				role: 'user',
				content,
				reasoning_content: null,
				model_id: this.selectedModel.model_id,
				reasoning_effort: this.reasoningEffort,
				input_tokens: null,
				output_tokens: null,
				reasoning_tokens: null,
				input_cost_usd: null,
				output_cost_usd: null,
				reasoning_cost_usd: null,
				total_cost_usd: null,
				latency_ms: null,
				reasoning_latency_ms: null,
				created_at: new Date().toISOString(),
			};
			this.messages.push(userMessage);

			// Create a placeholder assistant message for streaming
			const streamingMessageId = `streaming-${Date.now()}`;
			const streamingMessage: ChatMessage = {
				id: streamingMessageId,
				role: 'assistant',
				content: '',
				reasoning_content: null,
				model_id: this.selectedModel.model_id,
				reasoning_effort: this.reasoningEffort,
				input_tokens: null,
				output_tokens: null,
				reasoning_tokens: null,
				input_cost_usd: null,
				output_cost_usd: null,
				reasoning_cost_usd: null,
				total_cost_usd: null,
				latency_ms: null,
				reasoning_latency_ms: null,
				created_at: new Date().toISOString(),
			};
			this.messages.push(streamingMessage);

			try {
				const config = useRuntimeConfig();
				const baseUrl = config.public.apiBase || '';

				const response = await fetch(`${baseUrl}/api/v1/chats/${chatId}/stream`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
					},
					credentials: 'include',
					body: JSON.stringify({
						content,
						model_key: this.selectedModel.model_id,
						reasoning_effort: this.reasoningEffort || undefined,
						tools_enabled: this.enabledTools.length > 0 ? this.enabledTools : undefined,
					}),
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

					// Process complete SSE events
					const lines = buffer.split('\n');
					buffer = lines.pop() || '';

					for (const line of lines) {
						if (line.startsWith('data: ')) {
							const jsonStr = line.slice(6);
							if (!jsonStr) continue;

							try {
								const data = JSON.parse(jsonStr);
								const userMsgIndex = this.messages.findIndex(m => m.id === userMessageId);
								const msgIndex = this.messages.findIndex(m => m.id === streamingMessageId);
								const msg = msgIndex !== -1 ? this.messages[msgIndex] : null;

								switch (data.type) {
									case 'user_message_saved':
										// Update user message with real ID
										if (userMsgIndex !== -1) {
											const userMsg = this.messages[userMsgIndex];
											if (userMsg) userMsg.id = data.message_id;
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
									case 'tokens':
										if (msg) {
											msg.input_tokens = data.input;
											msg.output_tokens = data.output;
											msg.reasoning_tokens = data.reasoning || null;
										}
										this.contextTokens = data.input + data.output;
										break;
									case 'done':
										if (msg) {
											msg.id = data.message_id;
											msg.input_tokens = data.input_tokens;
											msg.output_tokens = data.output_tokens;
											msg.reasoning_tokens = data.reasoning_tokens || null;
											msg.latency_ms = data.latency_ms;
											msg.reasoning_latency_ms = data.reasoning_latency_ms || null;
										}
										this.isStreaming = false;

										// Update chat in list (2 messages added: user + assistant)
										const chatIndex = this.chats.findIndex(c => c.id === chatId);
										const chat = this.chats[chatIndex];
										if (chat) {
											chat.message_count += 2;
											chat.updated_at = new Date().toISOString();
										}
										break;
									case 'error':
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
				// Remove placeholders on error
				this.messages = this.messages.filter(m => m.id !== userMessageId && m.id !== streamingMessageId);
			}
		},

		// ===== Preferences =====
		async fetchPreferences() {
			this.preferencesLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const prefs = await $customFetch('/api/v1/users/@me/preferences');
				this.preferences = prefs as UserPreferences;
			} catch (e) {
				console.error('Failed to fetch preferences:', e);
				this.preferences = {...defaultPreferences};
			} finally {
				this.preferencesLoading = false;
			}
		},

		async updatePreferences(data: UpdatePreferencesRequest): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const updated = await $customFetch('/api/v1/users/@me/preferences', {
					method: 'PATCH',
					body: data,
				});
				this.preferences = updated as UserPreferences;
				return true;
			} catch (e) {
				console.error('Failed to update preferences:', e);
				return false;
			}
		},

		// ===== Models =====
		async fetchModels() {
			this.modelsLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const models = await $customFetch('/api/v1/models');
				this.models = models as Model[];

				// If no model selected, try to select default or first available
				if (!this.selectedModel && this.models.length > 0) {
					const defaultKey = this.preferences.default_model_key;
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

			// Auto-set reasoning effort based on model capabilities
			if (!model) {
				this.reasoningEffort = null;
				return;
			}

			const hasNone = model.capabilities.some(c => c === 'REASONING_EFFORT_NONE');
			const hasReasoningCap = model.capabilities.some(c => c.startsWith('REASONING_'));

			if (!hasReasoningCap) {
				// No reasoning capability
				this.reasoningEffort = null;
			} else if (!hasNone) {
				// Has reasoning but no NONE option - set to lowest available
				const efforts = model.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
				const effortOrder = ['MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
				const lowestEffort = effortOrder.find(e => efforts.includes(e));
				this.reasoningEffort = lowestEffort ? lowestEffort.toLowerCase() : null;
			} else {
				// Has NONE option - don't select reasoning by default
				this.reasoningEffort = null;
			}
		},

		async toggleFavoriteModel(modelKey: string): Promise<boolean> {
			const favorites = [...this.preferences.favorite_model_keys];
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

		setReasoningEffort(effort: string | null) {
			this.reasoningEffort = effort;
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

		// ===== Initialization =====
		async init() {
			await Promise.all([this.fetchWorkspaces(), this.fetchChats(), this.fetchPreferences()]);
			// Fetch models after preferences so we can select default model
			await this.fetchModels();
		},
	},
});
