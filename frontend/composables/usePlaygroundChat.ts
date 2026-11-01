export interface PlaygroundMetrics {
	requestId: string | null;
	ttftMs: number | null;
	totalMs: number | null;
	inputTokens: number | null;
	outputTokens: number | null;
	finishReason: string | null;
	stopped: boolean;
}

export interface PlaygroundMessage {
	role: 'user' | 'assistant';
	content: string;
	error?: string;
	metrics?: PlaygroundMetrics;
}

export interface PlaygroundSettings {
	model: string;
	system: string;
	temperature: string | number;
	maxTokens: string | number;
	stream: boolean;
}

function optionalNumber(value: string | number): number | undefined {
	return value === '' || value === null || Number.isNaN(Number(value)) ? undefined : Number(value);
}

export function usePlaygroundChat(settings: PlaygroundSettings, failureText: () => string) {
	const messages = ref<PlaygroundMessage[]>([]);
	const running = ref(false);
	const lastRequest = ref<Record<string, unknown> | null>(null);
	const lastHeaders = ref<[string, string][]>([]);
	let controller: AbortController | null = null;

	const requestBody = computed(() => {
		const history = messages.value.filter(message => !message.error && message.content !== '').map(({role, content}) => ({role, content}));
		const system = settings.system.trim();
		return {
			model: settings.model,
			messages: system ? [{role: 'system', content: system}, ...history] : history,
			temperature: optionalNumber(settings.temperature),
			max_tokens: optionalNumber(settings.maxTokens),
			stream: settings.stream,
		};
	});

	function applyChunk(reply: PlaygroundMessage, startedAt: number, chunk: any) {
		const metrics = reply.metrics as PlaygroundMetrics;
		const choice = chunk.choices?.[0];
		const text = choice?.delta?.content ?? choice?.message?.content ?? '';
		if (text && metrics.ttftMs === null && settings.stream) metrics.ttftMs = Math.round(performance.now() - startedAt);
		reply.content += text;
		metrics.finishReason = choice?.finish_reason ?? metrics.finishReason;
		metrics.inputTokens = chunk.usage?.prompt_tokens ?? metrics.inputTokens;
		metrics.outputTokens = chunk.usage?.completion_tokens ?? metrics.outputTokens;
	}

	async function readStream(response: Response, onChunk: (chunk: any) => void) {
		const reader = response.body?.getReader();
		if (!reader) return;
		const decoder = new TextDecoder();
		const handleLine = (line: string) => {
			const data = line.startsWith('data:') ? line.slice(5).trim() : '';
			if (!data || data === '[DONE]') return;
			try {
				onChunk(JSON.parse(data));
			} catch {
				return;
			}
		};
		let buffer = '';
		for (;;) {
			const {done, value} = await reader.read();
			if (done) break;
			buffer += decoder.decode(value, {stream: true});
			const lines = buffer.split('\n');
			buffer = lines.pop() ?? '';
			lines.forEach(handleLine);
		}
		handleLine(buffer);
	}

	async function run(token: string) {
		const body = requestBody.value;
		messages.value.push({role: 'assistant', content: '', metrics: {requestId: null, ttftMs: null, totalMs: null, inputTokens: null, outputTokens: null, finishReason: null, stopped: false}});
		const reply = messages.value[messages.value.length - 1];
		const metrics = reply.metrics as PlaygroundMetrics;
		const startedAt = performance.now();
		controller = new AbortController();
		running.value = true;
		lastRequest.value = body;
		lastHeaders.value = [];
		try {
			const response = await fetch(`${gatewayBaseUrl()}/chat/completions`, {
				method: 'POST',
				signal: controller.signal,
				headers: {'Content-Type': 'application/json', Authorization: `Bearer ${token}`},
				body: JSON.stringify(body.stream ? {...body, stream_options: {include_usage: true}} : body),
			});
			metrics.requestId = response.headers.get('x-request-id');
			lastHeaders.value = [...response.headers.entries()].filter(([name]) => name === 'x-request-id' || name.includes('ratelimit') || name === 'retry-after');
			if (!response.ok) {
				const failure = await response.json().catch(() => null);
				reply.error = failure?.error?.message ?? failure?.message ?? `HTTP ${response.status}`;
			} else if (body.stream) {
				await readStream(response, chunk => applyChunk(reply, startedAt, chunk));
			} else {
				applyChunk(reply, startedAt, await response.json());
			}
		} catch (e: any) {
			if (e?.name === 'AbortError') metrics.stopped = true;
			else reply.error = e?.message ?? failureText();
		} finally {
			metrics.totalMs = Math.round(performance.now() - startedAt);
			running.value = false;
			controller = null;
		}
	}

	function send(prompt: string, token: string) {
		messages.value.push({role: 'user', content: prompt});
		return run(token);
	}

	function regenerate(token: string) {
		if (messages.value[messages.value.length - 1]?.role === 'assistant') messages.value.pop();
		return run(token);
	}

	function stop() {
		controller?.abort();
	}

	function clear() {
		stop();
		messages.value = [];
		lastRequest.value = null;
		lastHeaders.value = [];
	}

	return {messages, running, requestBody, lastRequest, lastHeaders, send, regenerate, stop, clear};
}
