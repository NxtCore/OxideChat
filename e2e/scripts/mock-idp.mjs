import http from 'node:http';

const port = Number(process.env.MOCK_IDP_PORT ?? 3104);
const origin = `http://127.0.0.1:${port}`;
const ACCESS_TOKEN = 'mock-idp-access-token';

let profile = {sub: 'idp-user-1', email: 'idp-user@e2e.test', email_verified: true, name: 'Idp User'};
let lastTokenRequest = null;

function readBody(req) {
	return new Promise(resolve => {
		const chunks = [];
		req.on('data', chunk => chunks.push(chunk));
		req.on('end', () => resolve(Buffer.concat(chunks).toString()));
	});
}

function json(res, status, body) {
	res.writeHead(status, {'content-type': 'application/json'});
	res.end(JSON.stringify(body));
}

const server = http.createServer(async (req, res) => {
	const url = new URL(req.url ?? '/', origin);

	if (url.pathname === '/health') return json(res, 200, {ok: true});

	if (url.pathname === '/.well-known/openid-configuration') {
		return json(res, 200, {issuer: origin, authorization_endpoint: `${origin}/authorize`, token_endpoint: `${origin}/token`, userinfo_endpoint: `${origin}/userinfo`});
	}

	if (url.pathname === '/authorize') {
		const target = new URL(url.searchParams.get('redirect_uri') ?? '');
		target.searchParams.set('code', 'mock-code');
		target.searchParams.set('state', url.searchParams.get('state') ?? '');
		res.writeHead(302, {location: target.toString()});
		return res.end();
	}

	if (url.pathname === '/token' && req.method === 'POST') {
		const form = new URLSearchParams(await readBody(req));
		lastTokenRequest = {grant_type: form.get('grant_type'), code: form.get('code'), has_code_verifier: Boolean(form.get('code_verifier')), authorization: req.headers.authorization ?? null};
		if (form.get('code') !== 'mock-code' || !form.get('code_verifier')) return json(res, 400, {error: 'invalid_grant'});
		return json(res, 200, {access_token: ACCESS_TOKEN, token_type: 'Bearer', expires_in: 3600});
	}

	if (url.pathname === '/userinfo') {
		if (req.headers.authorization !== `Bearer ${ACCESS_TOKEN}`) return json(res, 401, {error: 'invalid_token'});
		return json(res, 200, profile);
	}

	if (url.pathname === '/profile' && req.method === 'POST') {
		profile = JSON.parse(await readBody(req));
		return json(res, 200, profile);
	}

	if (url.pathname === '/last-token-request') return json(res, 200, lastTokenRequest);

	json(res, 404, {error: 'not_found'});
});

server.listen(port, '127.0.0.1', () => console.log(`[mock-idp] listening on ${origin}`));
