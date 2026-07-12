import {defineStore} from 'pinia';
import {useMainStore} from './index';
import {useThemeStore} from './theme';
import {useBudgetStore} from './budgetStore';
import type {
	McpServer,
	Workspace,
	Chat,
	ChatMessage,
	ChatWithMessages,
	UserPreferences,
	ModelList,
	CreateWorkspaceRequest,
	UpdateWorkspaceRequest,
	DeleteWorkspaceOptions,
	CreateChatRequest,
	UpdateChatRequest,
	UpdatePreferencesRequest,
	ChatListParams,
	StreamingAnimation,
	PaginatedResponse,
} from '~/types/chat';

function parseSseJsonRpc(body: string): any {
	for (const line of body.split('\n')) {
		if (line.startsWith('data:')) {
			const payload = line.slice(5).trim();
			if (payload) {
				try {
					return JSON.parse(payload);
				} catch {}
			}
		}
	}
	throw new Error('No JSON-RPC response found in SSE stream');
}

const ACTIVE_WORKSPACE_KEY = 'oxide-active-workspace';

function loadActiveWorkspaceId(): string | null {
	if (typeof window === 'undefined') return null;
	try {
		return localStorage.getItem(ACTIVE_WORKSPACE_KEY);
	} catch {
		return null;
	}
}

function persistActiveWorkspaceId(id: string | null) {
	if (typeof window === 'undefined') return;
	try {
		if (id) localStorage.setItem(ACTIVE_WORKSPACE_KEY, id);
		else localStorage.removeItem(ACTIVE_WORKSPACE_KEY);
	} catch {}
}

interface ChatState {
	workspaces: Workspace[];
	activeWorkspaceId: string | null;
	workspacesLoading: boolean;

	mcpManagerOpen: boolean;
	userMcpServers: McpServer[];

	chats: Chat[];
	activeChat: Chat | null;
	chatsLoading: boolean;

	messages: ChatMessage[];
	messagesLoading: boolean;

	models: ModelList[];
	selectedModel: ModelList | null;
	modelsLoading: boolean;

	isStreaming: boolean;
	contextTokens: number;
	reasoningEffort: string | null;
	reasoningBudget: number | null;
	enabledTools: string[];

	// Upstream-provider selection (OpenRouter routing), gated by the instance setting.
	selectedProviderSlug: string | null;
	providerRoutingMode: 'prefer' | 'lock';

	// Branch prefill state
	pendingBranchContent: string | null;
	pendingBranchParts: any[] | null;

	initialized: boolean;
}

export const useChatStore = defineStore('chat', {
	state: (): ChatState => ({
		workspaces: [],
		activeWorkspaceId: null,
		workspacesLoading: false,

		mcpManagerOpen: false,
		userMcpServers: [],

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

		selectedProviderSlug: null,
		providerRoutingMode: 'prefer',

		pendingBranchContent: null,
		pendingBranchParts: null,

		initialized: false,
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

		favoriteModels(): ModelList[] {
			return this.models.filter(m => m.is_favorite);
		},

		groupedModels(): Record<string, ModelList[]> {
			const grouped: Record<string, ModelList[]> = {};
			for (const model of this.models) {
				const provider = model.provider_name;
				if (!grouped[provider]) grouped[provider] = [];
				grouped[provider].push(model);
			}
			return grouped;
		},

		hasReasoningCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.some(c => c === 'REASONING' || c.startsWith('REASONING_'));
		},

		hasConfigurableReasoningCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.some(c => c.startsWith('REASONING_EFFORT_') || c.startsWith('REASONING_BUDGET_TOKENS_'));
		},

		hasToolCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.includes('TOOLS');
		},

		hasVisionCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.input_modalities.includes('IMAGE');
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

		isFavoriteModel: _state => (model: ModelList) => {
			return model.is_favorite;
		},
	},

	actions: {
		async fetchUserMcpServers() {
			try {
				const {$customFetch} = useNuxtApp();
				const result = await $customFetch('/api/v1/mcp-servers');
				this.userMcpServers = (result as McpServer[]) ?? [];
			} catch (e) {
				console.error('Failed to fetch user MCP servers:', e);
			}
		},

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
				this.notifyWorkspaceError(e, 'workspace.create_failed');
				return null;
			}
		},

		notifyWorkspaceError(e: any, fallbackKey: string) {
			const mainStore = useMainStore();
			const message = e?.data?.errors?.[0]?.message || mainStore.getTranslation(fallbackKey);
			mainStore.toast(message, {type: 'error'});
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
				if (this.activeWorkspaceId === id) this.applyWorkspaceAccent();
				return true;
			} catch (e) {
				console.error('Failed to update workspace:', e);
				this.notifyWorkspaceError(e, 'workspace.update_failed');
				return false;
			}
		},

		async deleteWorkspace(id: string, opts: DeleteWorkspaceOptions): Promise<boolean> {
			try {
				const {$customFetch} = useNuxtApp();
				const query = new URLSearchParams({action: opts.action});
				if (opts.action === 'move' && opts.target_workspace_id) query.set('target_workspace_id', opts.target_workspace_id);
				await $customFetch(`/api/v1/workspaces/${id}?${query.toString()}`, {method: 'DELETE'});
				this.workspaces = this.workspaces.filter(w => w.id !== id);
				if (this.activeWorkspaceId === id) {
					this.activeWorkspaceId = null;
					persistActiveWorkspaceId(null);
					this.applyWorkspaceAccent();
				}
				await this.fetchWorkspaces();
				await this.fetchChats({workspace_id: this.activeWorkspaceId || undefined});
				return true;
			} catch (e) {
				console.error('Failed to delete workspace:', e);
				this.notifyWorkspaceError(e, 'workspace.delete_failed');
				return false;
			}
		},

		setActiveWorkspace(id: string | null) {
			this.activeWorkspaceId = id;
			persistActiveWorkspaceId(id);
			this.applyWorkspaceAccent();
			this.fetchChats({workspace_id: id || undefined});
		},

		applyWorkspaceAccent() {
			const active = this.activeWorkspaceId ? this.workspaces.find(w => w.id === this.activeWorkspaceId) : undefined;
			useThemeStore().setWorkspaceAccent(active?.color ?? null);
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
				await this.hydrateComposerFromMessages(this.messages);
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

		async setActiveChat(chat: Chat | null) {
			if (chat) {
				this.fetchChat(chat.id);
			} else {
				this.messages = [];
				this.setContextTokens(0);
				this.activeChat = null;
				if (this.initialized) await this.resetComposerToDefaults();
			}
		},

		async resetComposerToDefaults() {
			const prefs = useMainStore().preferences;
			const key = prefs?.effective_default_model_key;
			const model = key ? await this.findModelByKey(key) : (this.models[0] ?? null);
			this.setSelectedModel(model);
			this.selectedProviderSlug = prefs?.default_provider_slug ?? null;
			this.providerRoutingMode = 'prefer';
			this.enabledTools = prefs?.default_tools ? [...prefs.default_tools] : [];
		},

		async callLocalMcpTool(mcp_server_id: string, mcp_tool_name: string, args: any): Promise<any> {
			const server = this.userMcpServers.find(s => s.id === mcp_server_id);
			if (!server) throw new Error(`MCP server ${mcp_server_id} not found in local server list`);

			const url: string = server.connection_config?.url;
			const headers: Record<string, string> = server.connection_config?.headers ?? {};
			if (!url) throw new Error(`MCP server ${server.name} has no URL configured`);

			const response = await fetch(url, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					Accept: 'application/json, text/event-stream',
					'MCP-Protocol-Version': '2025-06-18',
					...headers,
				},
				body: JSON.stringify({
					jsonrpc: '2.0',
					id: Date.now(),
					method: 'tools/call',
					params: {name: mcp_tool_name, arguments: args},
				}),
			});

			if (!response.ok) {
				throw new Error(`MCP call failed: HTTP ${response.status}`);
			}

			const rawText = await response.text();
			const contentType = response.headers.get('content-type') ?? '';
			let data: any;
			if (contentType.includes('text/event-stream')) {
				data = parseSseJsonRpc(rawText);
			} else {
				data = JSON.parse(rawText);
			}

			if (data.error) throw new Error(data.error.message ?? 'MCP tool error');

			const content = data.result?.content ?? [];
			const text = content
				.filter((c: any) => c.type === 'text')
				.map((c: any) => c.text)
				.join('\n');
			return {result: text};
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
					client_id: userMessageId,
					role: 'user',
					content,
					reasoning_content: null,
					model_id: this.selectedModel.model_id,
					model_key: this.selectedModel.model_id,
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
					request_settings: {
						model_key: this.selectedModel.model_id,
						provider_slug: this.selectedProviderSlug,
						provider_routing_mode: this.providerRoutingMode,
						enabled_tools: [...this.enabledTools],
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
				client_id: streamingMessageId,
				role: 'assistant',
				content: '',
				reasoning_content: null,
				model_id: this.selectedModel.model_id,
				model_key: this.selectedModel.model_id,
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
				request_settings: {
					model_key: this.selectedModel.model_id,
					provider_slug: this.selectedProviderSlug,
					provider_routing_mode: this.providerRoutingMode,
					enabled_tools: [...this.enabledTools],
				},
				tool_calls: [],
				content_parts: [],
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

				if (this.selectedProviderSlug) {
					body.provider_slug = this.selectedProviderSlug;
					body.provider_routing_mode = this.providerRoutingMode;
				}

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
												userMsg.client_id = data.message.id;
											}
										}
										break;
									case 'text_delta':
										if (msg) {
											msg.content += data.content;
											if (!msg.content_parts) msg.content_parts = [];
											const lastPart = msg.content_parts[msg.content_parts.length - 1];
											if (lastPart && lastPart.type === 'text') {
												lastPart.text = (lastPart.text || '') + data.content;
											} else {
												msg.content_parts.push({type: 'text', text: data.content});
											}
										}
										break;
									case 'reasoning_delta':
										if (msg) {
											if (msg.reasoning_content === null) msg.reasoning_content = '';
											msg.reasoning_content += data.content;
											if (!msg.content_parts) msg.content_parts = [];
											const lastReasoningPart = msg.content_parts[msg.content_parts.length - 1];
											if (lastReasoningPart && lastReasoningPart.type === 'reasoning') {
												lastReasoningPart.text = (lastReasoningPart.text || '') + data.content;
											} else {
												msg.content_parts.push({type: 'reasoning', text: data.content});
											}
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
											if (!msg.content_parts) msg.content_parts = [];
											msg.content_parts.push({type: 'tool_call', id: data.id, name: data.name});
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
											if (msg) {
												msg.tool_calls.push(newToolCall as any);
												if (!msg.content_parts) msg.content_parts = [];
												if (!msg.content_parts.some(p => p.type === 'tool_call' && p.id === data.id)) {
													msg.content_parts.push({type: 'tool_call', id: data.id, name: data.name});
												}
											}
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
										const imageId = data.output?.image_id;
										if (
											msg &&
											!data.error &&
											(data.tool_name === 'imagegen' || data.tool_name === 'imagegen_generate') &&
											typeof imageId === 'string'
										) {
											if (!msg.content_parts) msg.content_parts = [];
											if (!msg.content_parts.some(part => part.type === 'image' && part.image_id === imageId)) {
												msg.content_parts.push({type: 'image', image_id: imageId});
											}
										}
										break;
									}
									case 'client_tool_call': {
										const config = useRuntimeConfig();
										const apiBase = config.public.apiBase || '';
										let result: any;
										let hasError = false;
										try {
											result = await this.callLocalMcpTool(data.mcp_server_id, data.mcp_tool_name, data.args);
										} catch (e: any) {
											result = {error: e?.message ?? 'Client MCP tool failed'};
											hasError = true;
										}
										try {
											await fetch(`${apiBase}/api/v1/chats/${chatId}/stream/tool-result`, {
												method: 'POST',
												headers: {'Content-Type': 'application/json'},
												credentials: 'include',
												body: JSON.stringify({call_id: data.id, result}),
											});
										} catch (e) {
											console.error('Failed to submit client tool result:', e);
										}
										if (msg && !hasError) {
											const toolCall = msg.tool_calls?.find(tc => tc.tool_call_id === data.id);
											if (toolCall) {
												toolCall.output = JSON.stringify(result);
											}
										}
										break;
									}
									case 'tokens':
										if (msg) {
											msg.usage_details.input_tokens = data.input;
											msg.usage_details.output_tokens = data.output;
											msg.usage_details.reasoning_tokens = data.reasoning;
										}
										this.contextTokens = data.input + data.output;
										break;
									case 'cost':
										if (msg) {
											Object.assign(msg.cost_details, data.cost_details);
										}
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
										const budgetStore = useBudgetStore();
										budgetStore.fetchMyBudget().catch(() => {});
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
				const models = await $customFetch<PaginatedResponse<ModelList>>('/api/v1/models?size=100');
				this.models = models.items;

				if (!this.selectedModel && this.models.length > 0) {
					await this.resetComposerToDefaults();
				}
			} catch (e) {
				console.error('Failed to fetch models:', e);
			} finally {
				this.modelsLoading = false;
			}
		},

		async findModelByKey(modelKey: string): Promise<ModelList | null> {
			let model = this.models.find(m => m.model_id === modelKey) ?? null;
			if (model) return model;

			if (this.models.length === 0) {
				await this.fetchModels();
				model = this.models.find(m => m.model_id === modelKey) ?? null;
				if (model) return model;
			}

			try {
				const {$customFetch} = useNuxtApp();
				const result = await $customFetch<PaginatedResponse<ModelList>>('/api/v1/models', {
					params: {
						query: modelKey,
						size: '10',
					},
				});
				const exact = result.items.find(m => m.model_id === modelKey) ?? null;
				if (exact && !this.models.some(m => m.id === exact.id)) {
					this.models.push(exact);
				}
				return exact;
			} catch (e) {
				console.error('Failed to fetch model for chat hydration:', e);
				return null;
			}
		},

		async hydrateComposerFromMessages(messages: ChatMessage[]) {
			const source = messages.findLast(m => m.role === 'assistant') ?? messages.findLast(m => m.role === 'user');
			if (!source) return;

			const settings = source.request_settings ?? {};
			const modelKey = source.model_key ?? settings.model_key ?? null;
			if (modelKey) {
				const model = await this.findModelByKey(modelKey);
				if (model && model.id !== this.selectedModel?.id) {
					this.setSelectedModel(model);
				}
			}

			this.reasoningEffort = source.reasoning_details?.effort ?? null;
			this.reasoningBudget = source.reasoning_details?.budget_tokens ?? null;
			this.selectedProviderSlug = settings.provider_slug ?? null;
			this.providerRoutingMode = settings.provider_routing_mode === 'lock' ? 'lock' : 'prefer';
			this.enabledTools = Array.isArray(settings.enabled_tools) ? [...settings.enabled_tools] : [];
		},

		setProviderSelection(slug: string | null, mode?: 'prefer' | 'lock') {
			this.selectedProviderSlug = slug;
			if (mode) this.providerRoutingMode = mode;
		},

		setSelectedModel(model: ModelList | null) {
			const modelChanged = this.selectedModel?.id !== model?.id;
			this.selectedModel = model;

			if (modelChanged) {
				const prefs = useMainStore().preferences;
				if (model && prefs?.default_model_key && model.model_id === prefs.default_model_key) {
					this.selectedProviderSlug = prefs.default_provider_slug ?? null;
				} else {
					this.selectedProviderSlug = null;
				}
			}

			if (!model) {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
				return;
			}

			const hasNone = model.capabilities.some(c => c === 'REASONING_EFFORT_NONE');
			const hasReasoningCap = model.capabilities.some(c => c === 'REASONING' || c.startsWith('REASONING_'));

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

		async toggleFavoriteModel(modelDbId: string): Promise<boolean> {
			const model = this.models.find(m => m.id === modelDbId);
			const isFavorite = !(model?.is_favorite ?? false);

			if (model) model.is_favorite = isFavorite;

			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch(`/api/v1/models/${modelDbId}/favorite`, {
					method: 'POST',
					body: {is_favorite: isFavorite},
				});
				return true;
			} catch (e) {
				if (model) model.is_favorite = !isFavorite;
				console.error('Failed to toggle model favorite:', e);
				return false;
			}
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
				// Reload the chat to get correct fork path
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
			this.initialized = false;
			this.activeWorkspaceId = loadActiveWorkspaceId();
			await Promise.all([this.fetchWorkspaces(), this.fetchPreferences(), this.fetchUserMcpServers()]);
			if (this.activeWorkspaceId && !this.workspaces.some(w => w.id === this.activeWorkspaceId)) {
				this.activeWorkspaceId = null;
				persistActiveWorkspaceId(null);
			}
			await this.fetchChats({workspace_id: this.activeWorkspaceId || undefined});
			this.applyWorkspaceAccent();
			await this.fetchModels();
			if (!this.activeChat) await this.resetComposerToDefaults();
			this.initialized = true;
		},
	},
});
