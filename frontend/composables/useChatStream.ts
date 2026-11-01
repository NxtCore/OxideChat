import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {usePreferencesStore} from '~/stores/preferencesStore';
import {useMainStore} from '~/stores';
import {useBudgetStore} from '~/stores/budgetStore';
import type {ChatMessage} from '~/types/chat';

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

async function callLocalMcpTool(preferencesStore: ReturnType<typeof usePreferencesStore>, serverId: string, toolName: string, args: any): Promise<any> {
	const server = preferencesStore.userMcpServers.find(s => s.id === serverId);
	if (!server) throw new Error(`MCP server ${serverId} not found in local server list`);

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
			params: {name: toolName, arguments: args},
		}),
	});

	if (!response.ok) {
		throw new Error(`MCP call failed: HTTP ${response.status}`);
	}

	const rawText = await response.text();
	const contentType = response.headers.get('content-type') ?? '';
	const data = contentType.includes('text/event-stream') ? parseSseJsonRpc(rawText) : JSON.parse(rawText);

	if (data.error) throw new Error(data.error.message ?? 'MCP tool error');

	const content = data.result?.content ?? [];
	const text = content
		.filter((c: any) => c.type === 'text')
		.map((c: any) => c.text)
		.join('\n');
	return {result: text};
}

function newMessage(modelStore: ReturnType<typeof useModelStore>, id: string, role: 'user' | 'assistant', content: string): ChatMessage {
	const modelKey = modelStore.selectedModel?.model_id ?? '';
	return {
		id,
		client_id: id,
		role,
		content,
		reasoning_content: null,
		model_id: modelKey,
		model_key: modelKey,
		cost_details: {input: null, output: null, reasoning: null},
		usage_details: {input_tokens: null, output_tokens: null, reasoning_tokens: null, latency_ms: null, reasoning_latency_ms: null},
		reasoning_details: {effort: modelStore.reasoningEffort, budget_tokens: modelStore.reasoningBudget},
		request_settings: {
			model_key: modelKey,
			provider_slug: modelStore.selectedProviderSlug,
			provider_routing_mode: modelStore.providerRoutingMode,
			enabled_tools: [...modelStore.enabledTools],
		},
		tool_calls: [],
		...(role === 'assistant' ? {content_parts: []} : {}),
		created_at: new Date().toISOString(),
		parent_id: null,
		fork_index: 1,
		sibling_count: 1,
	};
}

export function useChatStream() {
	const chatStore = useChatStore();
	const modelStore = useModelStore();
	const preferencesStore = usePreferencesStore();

	function applyEvent(data: any, chatId: string, userMessageId: string | null, streamingMessageId: string): Promise<void> | void {
		const userMsg = userMessageId ? chatStore.messages.find(m => m.id === userMessageId) : undefined;
		const msg = chatStore.messages.find(m => m.id === streamingMessageId);

		switch (data.type) {
			case 'user_message_saved':
				if (userMsg) {
					Object.assign(userMsg, data.message);
					userMsg.client_id = data.message.id;
				}
				break;
			case 'text_delta':
			case 'reasoning_delta': {
				if (!msg) break;
				const kind = data.type === 'text_delta' ? 'text' : 'reasoning';
				if (kind === 'text') {
					msg.content += data.content;
				} else {
					msg.reasoning_content = (msg.reasoning_content ?? '') + data.content;
				}
				if (!msg.content_parts) msg.content_parts = [];
				const lastPart = msg.content_parts[msg.content_parts.length - 1];
				if (lastPart && lastPart.type === kind) {
					lastPart.text = (lastPart.text || '') + data.content;
				} else {
					msg.content_parts.push({type: kind, text: data.content});
				}
				break;
			}
			case 'tool_call_start':
				if (msg) {
					msg.tool_calls.push({tool_call_id: data.id, tool_name: data.name, input_args: ''} as any);
					if (!msg.content_parts) msg.content_parts = [];
					msg.content_parts.push({type: 'tool_call', id: data.id, name: data.name, arguments: ''});
				}
				break;
			case 'tool_call_delta': {
				const toolCall = msg?.tool_calls?.find(tc => tc.tool_call_id === data.id);
				if (toolCall && typeof toolCall.input_args === 'string') {
					toolCall.input_args += data.args_delta;
				} else if (msg) {
					msg.tool_calls.push({tool_call_id: data.id, tool_name: data.name, input_args: data.args_delta} as any);
					if (!msg.content_parts) msg.content_parts = [];
					if (!msg.content_parts.some(p => p.type === 'tool_call' && p.id === data.id)) {
						msg.content_parts.push({type: 'tool_call', id: data.id, name: data.name, arguments: data.args_delta ?? ''});
					}
				}
				break;
			}
			case 'tool_result': {
				const toolCall = msg?.tool_calls?.find(tc => tc.tool_call_id === data.id);
				if (toolCall) {
					toolCall.output = data.output ? JSON.stringify(data.output) : null;
					toolCall.error = data.error || null;
				}
				const imageId = data.output?.image_id;
				if (msg && !data.error && ['imagegen', 'imagegen_generate', 'imagegen_edit'].includes(data.tool_name) && typeof imageId === 'string') {
					if (!msg.content_parts) msg.content_parts = [];
					if (!msg.content_parts.some(part => part.type === 'image' && part.image_id === imageId)) {
						msg.content_parts.push({type: 'image', image_id: imageId});
					}
				}
				break;
			}
			case 'client_tool_call':
				return submitClientToolCall(data, chatId, msg);
			case 'tokens':
				if (msg) {
					msg.usage_details.input_tokens = data.input;
					msg.usage_details.output_tokens = data.output;
					msg.usage_details.reasoning_tokens = data.reasoning;
				}
				modelStore.contextTokens = data.input + data.output;
				break;
			case 'cost':
				if (msg) Object.assign(msg.cost_details, data.cost_details);
				break;
			case 'done': {
				if (msg) Object.assign(msg, data.message);
				chatStore.isStreaming = false;
				const chat = chatStore.chats.find(c => c.id === chatId);
				if (chat) {
					chat.message_count += 2;
					chat.updated_at = new Date().toISOString();
				}
				useBudgetStore()
					.fetchMyBudget()
					.catch(() => {});
				break;
			}
			case 'error':
				useMainStore().toast(data.message, {type: 'error'});
				console.error('Stream error:', data.code, data.message);
				if (msg) msg.content = `Error: ${data.message}`;
				chatStore.isStreaming = false;
				break;
		}
	}

	async function submitClientToolCall(data: any, chatId: string, msg: ChatMessage | undefined): Promise<void> {
		const apiBase = useRuntimeConfig().public.apiBase || '';
		let result: any;
		let hasError = false;
		try {
			result = await callLocalMcpTool(preferencesStore, data.mcp_server_id, data.mcp_tool_name, data.args);
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
		const toolCall = msg?.tool_calls?.find(tc => tc.tool_call_id === data.id);
		if (toolCall && !hasError) toolCall.output = JSON.stringify(result);
	}

	async function sendAndStream(chatId: string, content: string, parts?: any[], skipUserMessage: boolean = false, regenerateFromMessageId?: string): Promise<void> {
		const model = modelStore.selectedModel;
		if (!model) {
			console.error('No model selected');
			return;
		}

		chatStore.isStreaming = true;

		const userMessageId = skipUserMessage ? null : `user-${Date.now()}`;
		if (userMessageId) chatStore.messages.push(newMessage(modelStore, userMessageId, 'user', content));

		const streamingMessageId = `streaming-${Date.now()}`;
		chatStore.messages.push(newMessage(modelStore, streamingMessageId, 'assistant', ''));

		try {
			const baseUrl = useRuntimeConfig().public.apiBase || '';

			const body: any = {
				content,
				model_key: model.model_id,
				reasoning_effort: modelStore.reasoningEffort || undefined,
				reasoning_budget_tokens: modelStore.reasoningBudget || undefined,
				tools_enabled: modelStore.enabledTools.length > 0 ? modelStore.enabledTools : undefined,
				skip_user_message: skipUserMessage,
				regenerate_from_message_id: regenerateFromMessageId,
			};

			if (modelStore.selectedProviderSlug) {
				body.provider_slug = modelStore.selectedProviderSlug;
				body.provider_routing_mode = modelStore.providerRoutingMode;
			}

			if (parts && parts.length > 0) body.parts = parts;

			const response = await fetch(`${baseUrl}/api/v1/chats/${chatId}/stream`, {
				method: 'POST',
				headers: {'Content-Type': 'application/json'},
				credentials: 'include',
				body: JSON.stringify(body),
			});

			if (!response.ok) throw new Error(`Stream request failed: ${response.status}`);

			const reader = response.body?.getReader();
			if (!reader) throw new Error('No response body');

			const decoder = new TextDecoder();
			let buffer = '';

			while (true) {
				const {done, value} = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, {stream: true});
				const lines = buffer.split('\n');
				buffer = lines.pop() || '';

				for (const line of lines) {
					if (!line.startsWith('data: ')) continue;
					const jsonStr = line.slice(6).trim();
					if (!jsonStr) continue;

					try {
						await applyEvent(JSON.parse(jsonStr), chatId, userMessageId, streamingMessageId);
					} catch (e) {
						console.error('Failed to parse SSE event:', e);
					}
				}
			}
		} catch (e) {
			console.error('Failed to stream:', e);
			chatStore.isStreaming = false;
			chatStore.messages = chatStore.messages.filter(m => m.id !== streamingMessageId && m.id !== userMessageId);
		}
	}

	return {sendAndStream};
}
