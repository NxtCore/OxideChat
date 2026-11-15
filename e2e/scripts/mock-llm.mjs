import http from 'node:http';
import {MEDIA_USAGE} from '../tests/support/constants.ts';

const port = Number(process.env.MOCK_LLM_PORT ?? 3103);

export const USAGE = {prompt_tokens: 1000, completion_tokens: 2000};
export const REPORTED_COST = 86;

const MODELS = ['mock-chat', 'mock-error', 'mock-slow'];
const MEDIA_MODELS = ['mock-embed', 'mock-rerank', 'mock-decide', 'mock-image', 'mock-whisper', 'mock-tts'];

const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
const BULK_MODELS = Array.from({length: 120}, (_, i) => `bulk-model-${String(i + 1).padStart(3, '0')}`);

function lastUserText(body) {
	const messages = body.messages ?? [];
	for (let i = messages.length - 1; i >= 0; i--) {
		const message = messages[i];
		if (message.role !== 'user') continue;
		if (typeof message.content === 'string') return message.content;
		if (Array.isArray(message.content)) {
			return message.content
				.filter(part => part.type === 'text')
				.map(part => part.text)
				.join(' ');
		}
	}
	return '';
}

function replyFor(body) {
	return `Mock reply: ${lastUserText(body)}`;
}

function usage() {
	return {...USAGE, total_tokens: USAGE.prompt_tokens + USAGE.completion_tokens, cost: REPORTED_COST};
}

function readBody(req) {
	return new Promise((resolve, reject) => {
		let data = '';
		req.on('data', chunk => (data += chunk));
		req.on('end', () => {
			try {
				resolve(data ? JSON.parse(data) : {});
			} catch (error) {
				reject(error);
			}
		});
		req.on('error', reject);
	});
}

function sendJson(res, status, value) {
	res.writeHead(status, {'content-type': 'application/json'});
	res.end(JSON.stringify(value));
}

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function readRaw(req) {
	return new Promise((resolve, reject) => {
		const chunks = [];
		req.on('data', chunk => chunks.push(chunk));
		req.on('end', () => resolve(Buffer.concat(chunks)));
		req.on('error', reject);
	});
}

function formFields(raw) {
	const fields = {};
	for (const match of raw.toString('latin1').matchAll(/name="([^"]+)"(?:; filename="[^"]*")?\r\n(?:Content-Type: [^\r]*\r\n)?\r\n/g)) {
		fields[match[1]] = [...(fields[match[1]] ?? []), match.index];
	}
	const text = name => {
		const at = fields[name]?.[0];
		if (at === undefined) return undefined;
		const rest = raw.toString('latin1').slice(at);
		const start = rest.indexOf('\r\n\r\n') + 4;
		return rest.slice(start, rest.indexOf('\r\n', start));
	};
	return {count: name => fields[name]?.length ?? 0, text};
}

function imageReply(count) {
	return {
		created: Math.floor(Date.now() / 1000),
		data: Array.from({length: count}, () => ({b64_json: PNG})),
		usage: {input_tokens: MEDIA_USAGE.imageInput, output_tokens: MEDIA_USAGE.imageOutput, total_tokens: MEDIA_USAGE.imageInput + MEDIA_USAGE.imageOutput},
	};
}

async function media(req, res, path) {
	if (req.method === 'GET' && path === '/v1/models') {
		return sendJson(res, 200, {object: 'list', data: MEDIA_MODELS.map(id => ({id, object: 'model', created: 0, owned_by: 'mock'}))});
	}
	if (req.method !== 'POST') return sendJson(res, 404, {error: {message: `mock: no media route for ${req.method} ${path}`}});
	const multipart = (req.headers['content-type'] ?? '').startsWith('multipart/form-data');
	const raw = multipart ? await readRaw(req) : undefined;
	const body = multipart ? {} : await readBody(req);
	const form = raw ? formFields(raw) : undefined;
	const model = form ? form.text('model') : body.model;
	if (!MEDIA_MODELS.includes(model)) {
		return sendJson(res, 400, {error: {message: `mock: unexpected model ${model}`, type: 'invalid_request_error', param: 'model'}});
	}
	switch (path) {
		case '/v1/embeddings': {
			const inputs = Array.isArray(body.input) ? body.input : [body.input];
			return sendJson(res, 200, {
				object: 'list',
				model,
				data: inputs.map((_, index) => ({object: 'embedding', index, embedding: [0.1, 0.2, 0.3]})),
				usage: {prompt_tokens: MEDIA_USAGE.embeddingTokens, total_tokens: MEDIA_USAGE.embeddingTokens},
			});
		}
		case '/v1/rerank':
			return sendJson(res, 200, {
				model,
				results: body.documents.map((_, index) => ({index, relevance_score: 1 / (index + 1)})),
				usage: {total_tokens: MEDIA_USAGE.rerankTokens, search_units: MEDIA_USAGE.searchUnits},
			});
		case '/v1/decisions':
			return sendJson(res, 200, {
				id: `dec-mock-${Date.now()}`,
				model,
				answers: body.questions.map(() => ({type: 'predicate', probability: 0.9})),
				usage: {input_tokens: MEDIA_USAGE.decisionTokens, output_tokens: 1},
			});
		case '/v1/images/generations':
			return sendJson(res, 200, imageReply(Number(body.n ?? 1)));
		case '/v1/images/edits':
			if (!form.count('image') && !form.count('image[]')) return sendJson(res, 400, {error: {message: "mock: missing 'image'", type: 'invalid_request_error'}});
			return sendJson(res, 200, imageReply(Number(form.text('n') ?? 1)));
		case '/v1/audio/transcriptions':
		case '/v1/audio/translations':
			if (!form?.count('file')) return sendJson(res, 400, {error: {message: "mock: missing 'file'", type: 'invalid_request_error'}});
			return sendJson(res, 200, {text: 'mock transcript', usage: {type: 'duration', seconds: MEDIA_USAGE.transcriptSeconds}});
		case '/v1/audio/speech':
			res.writeHead(200, {'content-type': 'audio/mpeg'});
			return res.end(Buffer.from(`ID3 mock speech: ${body.input}`));
		default:
			return sendJson(res, 404, {error: {message: `mock: no media route for ${req.method} ${path}`}});
	}
}

async function streamCompletion(req, res, body, model) {
	res.writeHead(200, {'content-type': 'text/event-stream', 'cache-control': 'no-cache', connection: 'keep-alive'});
	const id = `chatcmpl-mock-${Date.now()}`;
	const created = Math.floor(Date.now() / 1000);
	const chunk = (delta, finish_reason = null) => ({id, object: 'chat.completion.chunk', created, model, choices: [{index: 0, delta, finish_reason}]});
	const send = value => res.write(`data: ${JSON.stringify(value)}\n\n`);

	let closed = false;
	req.on('close', () => (closed = true));

	send(chunk({role: 'assistant', content: ''}));
	const words = replyFor(body).split(' ');
	const delay = model === 'mock-slow' ? 400 : 5;
	for (const [index, word] of words.entries()) {
		if (closed) return;
		await sleep(delay);
		send(chunk({content: index === 0 ? word : ` ${word}`}));
	}
	if (model === 'mock-slow') {
		for (let i = 0; i < 50 && !closed; i++) {
			await sleep(400);
			send(chunk({content: ' …'}));
		}
	}
	send(chunk({}, 'stop'));
	send({id, object: 'chat.completion.chunk', created, model, choices: [], usage: usage()});
	res.write('data: [DONE]\n\n');
	res.end();
}

const server = http.createServer(async (req, res) => {
	const url = new URL(req.url, `http://localhost:${port}`);
	if (req.method === 'GET' && url.pathname === '/health') return sendJson(res, 200, {ok: true});

	if (req.headers.authorization !== 'Bearer mock-key') {
		return sendJson(res, 401, {error: {message: 'Incorrect API key provided', type: 'invalid_request_error', code: 'invalid_api_key'}});
	}

	if (url.pathname.startsWith('/media/')) {
		try {
			return await media(req, res, url.pathname.slice('/media'.length));
		} catch {
			return sendJson(res, 400, {error: {message: 'mock: invalid request body', type: 'invalid_request_error'}});
		}
	}

	if (req.method === 'GET' && url.pathname === '/bulk/v1/models') {
		return sendJson(res, 200, {object: 'list', data: BULK_MODELS.map(id => ({id, object: 'model', created: 0, owned_by: 'mock'}))});
	}

	if (req.method === 'GET' && url.pathname === '/v1/models') {
		return sendJson(res, 200, {object: 'list', data: MODELS.map(id => ({id, object: 'model', created: 0, owned_by: 'mock'}))});
	}

	if (req.method === 'GET' && url.pathname === '/v1/providers') {
		return sendJson(res, 200, {
			data: [
				{name: 'Azure', slug: 'azure', headquarters: 'US', datacenters: ['US', 'SE']},
				{name: 'Mistral', slug: 'mistral', headquarters: 'FR', datacenters: ['FR']},
				{name: 'Together', slug: 'together', headquarters: 'US', datacenters: ['US']},
			],
		});
	}

	if (req.method === 'POST' && url.pathname === '/v1/chat/completions') {
		let body;
		try {
			body = await readBody(req);
		} catch {
			return sendJson(res, 400, {error: {message: 'invalid json', type: 'invalid_request_error'}});
		}
		const model = body.model ?? 'mock-chat';
		if (typeof body.temperature === 'number' && (body.temperature < 0 || body.temperature > 2)) {
			return sendJson(res, 400, {error: {message: `Invalid 'temperature': decimal above maximum value. Expected a value <= 2, but got ${body.temperature} instead.`, type: 'invalid_request_error', param: 'temperature'}});
		}
		if (model === 'mock-error') {
			return sendJson(res, 500, {error: {message: 'mock upstream failure', type: 'server_error'}});
		}
		if (body.stream) return streamCompletion(req, res, body, model);
		return sendJson(res, 200, {
			id: `chatcmpl-mock-${Date.now()}`,
			object: 'chat.completion',
			created: Math.floor(Date.now() / 1000),
			model,
			choices: [{index: 0, message: {role: 'assistant', content: replyFor(body)}, finish_reason: 'stop'}],
			usage: usage(),
		});
	}

	sendJson(res, 404, {error: {message: `mock: no route for ${req.method} ${url.pathname}`}});
});

server.listen(port, '127.0.0.1', () => console.log(`[mock-llm] listening on http://127.0.0.1:${port}`));
