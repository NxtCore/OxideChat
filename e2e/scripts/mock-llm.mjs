import http from 'node:http';

const port = Number(process.env.MOCK_LLM_PORT ?? 3103);

export const USAGE = {prompt_tokens: 1000, completion_tokens: 2000};

const MODELS = ['mock-chat', 'mock-error', 'mock-slow'];

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
	return {...USAGE, total_tokens: USAGE.prompt_tokens + USAGE.completion_tokens};
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

	if (req.method === 'GET' && url.pathname === '/v1/models') {
		return sendJson(res, 200, {object: 'list', data: MODELS.map(id => ({id, object: 'model', created: 0, owned_by: 'mock'}))});
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
